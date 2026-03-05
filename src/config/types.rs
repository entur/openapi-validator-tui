use std::collections::HashMap;
use std::fmt;

use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Server,
    Client,
    Both,
}

impl Mode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Mode::Server => "server",
            Mode::Client => "client",
            Mode::Both => "both",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Linter {
    Spectral,
    Redocly,
    None,
}

impl Linter {
    pub fn as_str(&self) -> &'static str {
        match self {
            Linter::Spectral => "spectral",
            Linter::Redocly => "redocly",
            Linter::None => "none",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Jobs {
    Auto,
    Fixed(usize),
}

impl Jobs {
    pub fn resolve(self) -> usize {
        match self {
            Jobs::Fixed(n) => n,
            Jobs::Auto => std::thread::available_parallelism()
                .map(|n| n.get().min(4))
                .unwrap_or(1),
        }
    }
}

impl Serialize for Jobs {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Jobs::Auto => serializer.serialize_str("auto"),
            Jobs::Fixed(n) => serializer.serialize_u64(*n as u64),
        }
    }
}

impl<'de> Deserialize<'de> for Jobs {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct JobsVisitor;

        impl<'de> Visitor<'de> for JobsVisitor {
            type Value = Jobs;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("\"auto\" or a positive integer")
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Jobs, E> {
                if value == 0 {
                    return Err(E::custom("jobs must be a positive integer"));
                }
                Ok(Jobs::Fixed(value as usize))
            }

            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Jobs, E> {
                if value <= 0 {
                    return Err(E::custom("jobs must be a positive integer"));
                }
                Ok(Jobs::Fixed(value as usize))
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Jobs, E> {
                if value.eq_ignore_ascii_case("auto") {
                    Ok(Jobs::Auto)
                } else {
                    Err(E::custom("jobs must be \"auto\" or a positive integer"))
                }
            }
        }

        deserializer.deserialize_any(JobsVisitor)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub spec: Option<String>,
    pub mode: Mode,
    pub lint: bool,
    pub generate: bool,
    pub compile: bool,
    pub linter: Linter,
    pub server_generators: Vec<String>,
    pub client_generators: Vec<String>,
    pub generator_config_overrides: HashMap<String, String>,
    pub generator_image: String,
    pub redocly_image: String,
    pub spectral_image: String,
    pub spectral_ruleset: String,
    pub spectral_fail_severity: String,
    pub docker_timeout: u64,
    pub search_depth: usize,
    pub jobs: Jobs,
    pub manage_gitignore: bool,
    #[serde(default, deserialize_with = "deserialize_keys")]
    pub keys: HashMap<String, Vec<String>>,
}

/// Accept both scalar strings and lists per action in the `keys` config map.
///
/// This allows users to write either form in `.oavc`:
/// ```yaml
/// keys:
///   scroll_down: "j"          # single string
///   quit: ["q", "C-c"]        # list of strings
///   toggle_diff: []            # explicit unbind
/// ```
fn deserialize_keys<'de, D>(deserializer: D) -> Result<HashMap<String, Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    struct KeysVisitor;

    impl<'de> Visitor<'de> for KeysVisitor {
        type Value = HashMap<String, Vec<String>>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a map of action names to key strings or lists of key strings")
        }

        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: de::MapAccess<'de>,
        {
            let mut result = HashMap::new();
            while let Some(key) = map.next_key::<String>()? {
                let value: StringOrVec = map.next_value()?;
                result.insert(key, value.into_vec());
            }
            Ok(result)
        }
    }

    deserializer.deserialize_map(KeysVisitor)
}

/// Helper for deserializing either a single string or a list of strings.
#[derive(Debug)]
enum StringOrVec {
    Single(String),
    Multiple(Vec<String>),
}

impl StringOrVec {
    fn into_vec(self) -> Vec<String> {
        match self {
            StringOrVec::Single(s) => vec![s],
            StringOrVec::Multiple(v) => v,
        }
    }
}

impl<'de> Deserialize<'de> for StringOrVec {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct StringOrVecVisitor;

        impl<'de> Visitor<'de> for StringOrVecVisitor {
            type Value = StringOrVec;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a string or a list of strings")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<StringOrVec, E> {
                Ok(StringOrVec::Single(value.to_owned()))
            }

            fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<StringOrVec, S::Error> {
                let mut v = Vec::new();
                while let Some(s) = seq.next_element::<String>()? {
                    v.push(s);
                }
                Ok(StringOrVec::Multiple(v))
            }
        }

        deserializer.deserialize_any(StringOrVecVisitor)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            spec: None,
            mode: Mode::Server,
            lint: true,
            generate: true,
            compile: true,
            linter: Linter::Spectral,
            server_generators: Vec::new(),
            client_generators: Vec::new(),
            generator_config_overrides: HashMap::new(),
            generator_image: "openapitools/openapi-generator-cli:v7.17.0".to_string(),
            redocly_image: "redocly/cli:1.25.5".to_string(),
            spectral_image: "stoplight/spectral:6".to_string(),
            spectral_ruleset:
                "https://raw.githubusercontent.com/entur/api-guidelines/refs/tags/v2/.spectral.yml"
                    .to_string(),
            spectral_fail_severity: "error".to_string(),
            docker_timeout: 300,
            search_depth: 4,
            jobs: Jobs::Auto,
            manage_gitignore: true,
            keys: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_config(yaml: &str) -> Config {
        serde_yaml::from_str(yaml).expect("should parse")
    }

    #[test]
    fn keys_scalar_string_per_action() {
        let cfg = parse_config(
            r#"
keys:
  scroll_down: "j"
  quit: "q"
"#,
        );
        assert_eq!(cfg.keys["scroll_down"], vec!["j"]);
        assert_eq!(cfg.keys["quit"], vec!["q"]);
    }

    #[test]
    fn keys_list_of_strings_per_action() {
        let cfg = parse_config(
            r#"
keys:
  quit: ["q", "C-c"]
  scroll_down: ["j", "Down"]
"#,
        );
        assert_eq!(cfg.keys["quit"], vec!["q", "C-c"]);
        assert_eq!(cfg.keys["scroll_down"], vec!["j", "Down"]);
    }

    #[test]
    fn keys_empty_list_unbinds() {
        let cfg = parse_config(
            r#"
keys:
  toggle_diff: []
"#,
        );
        assert!(cfg.keys["toggle_diff"].is_empty());
    }

    #[test]
    fn keys_mixed_scalar_and_list() {
        let cfg = parse_config(
            r#"
keys:
  scroll_down: "j"
  quit: ["q", "C-c"]
"#,
        );
        assert_eq!(cfg.keys["scroll_down"], vec!["j"]);
        assert_eq!(cfg.keys["quit"], vec!["q", "C-c"]);
    }

    #[test]
    fn keys_omitted_defaults_to_empty() {
        let cfg = parse_config("spec: petstore.yaml\n");
        assert!(cfg.keys.is_empty());
    }

    #[test]
    fn keys_invalid_yaml_type_is_rejected() {
        let result = serde_yaml::from_str::<Config>(
            r#"
keys:
  scroll_down: 42
"#,
        );
        assert!(result.is_err());
    }
}
