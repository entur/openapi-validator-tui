use std::collections::HashMap;
use std::fmt;

use serde::de::{self, Visitor};
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
    pub generator_overrides: HashMap<String, String>,
    pub generator_image: String,
    pub redocly_image: String,
    pub spectral_image: String,
    pub spectral_ruleset: String,
    pub spectral_fail_severity: String,
    pub docker_timeout: u64,
    pub search_depth: usize,
    pub jobs: Jobs,
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
            generator_overrides: HashMap::new(),
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
        }
    }
}
