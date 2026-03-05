use std::collections::HashSet;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::generators;

#[derive(Debug, Clone, Deserialize)]
pub struct CustomGeneratorDef {
    pub name: String,
    pub scope: String,
    pub generate: GenerateBlock,
    pub compile: Option<CompileBlock>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GenerateBlock {
    pub image: String,
    pub command: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CompileBlock {
    pub image: String,
    pub command: String,
}

/// Load custom generator definitions from YAML files in the given directory.
///
/// Returns an empty vec if the directory is missing or unreadable.
/// Validates names, scopes, required fields, and checks for collisions
/// with builtins and duplicates.
pub fn load(root: &Path, dir: &str) -> Result<Vec<CustomGeneratorDef>> {
    let custom_dir = root.join(dir);
    if !custom_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<_> = fs::read_dir(&custom_dir)
        .with_context(|| format!("failed to read {}", custom_dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("failed to iterate {}", custom_dir.display()))?;
    entries.sort_by_key(|e| e.file_name());

    let mut defs = Vec::new();
    for entry in entries {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str());
        if ext != Some("yaml") && ext != Some("yml") {
            continue;
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let def: CustomGeneratorDef = serde_yaml::from_str(&content)
            .with_context(|| format!("failed to parse {}", path.display()))?;

        validate_def(&def, &path)?;
        defs.push(def);
    }

    check_collisions(&defs)?;
    Ok(defs)
}

fn is_safe_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_' || c == '-')
}

fn validate_def(def: &CustomGeneratorDef, path: &Path) -> Result<()> {
    if def.name.trim().is_empty() {
        bail!("custom generator in {} has an empty name", path.display());
    }
    if !is_safe_name(&def.name) {
        bail!(
            "custom generator '{}' in {} has an invalid name \
             (must match [a-z0-9][a-z0-9._-]*)",
            def.name,
            path.display()
        );
    }
    match def.scope.as_str() {
        "server" | "client" => {}
        other => bail!(
            "custom generator '{}' has invalid scope '{other}' (expected server or client)",
            def.name,
        ),
    }
    if def.generate.image.trim().is_empty() {
        bail!(
            "custom generator '{}' has an empty generate.image",
            def.name
        );
    }
    if def.generate.command.trim().is_empty() {
        bail!(
            "custom generator '{}' has an empty generate.command",
            def.name
        );
    }
    if let Some(compile) = &def.compile {
        if compile.image.trim().is_empty() {
            bail!("custom generator '{}' has an empty compile.image", def.name);
        }
        if compile.command.trim().is_empty() {
            bail!(
                "custom generator '{}' has an empty compile.command",
                def.name
            );
        }
    }
    Ok(())
}

fn check_collisions(defs: &[CustomGeneratorDef]) -> Result<()> {
    let mut seen = HashSet::new();
    for def in defs {
        let builtins = generators::builtin_generators_for_scope(&def.scope);
        if builtins.iter().any(|g| g.name == def.name) {
            bail!(
                "custom generator '{}' collides with built-in {} generator",
                def.name,
                def.scope
            );
        }
        if !seen.insert((&def.name, &def.scope)) {
            bail!(
                "duplicate custom generator name '{}' for scope '{}'",
                def.name,
                def.scope
            );
        }
    }
    Ok(())
}

pub fn server_names(defs: &[CustomGeneratorDef]) -> Vec<String> {
    defs.iter()
        .filter(|d| d.scope == "server")
        .map(|d| d.name.clone())
        .collect()
}

pub fn client_names(defs: &[CustomGeneratorDef]) -> Vec<String> {
    defs.iter()
        .filter(|d| d.scope == "client")
        .map(|d| d.name.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_name_valid() {
        for name in ["my-gen", "gen.v2", "a", "1foo", "a_b"] {
            assert!(is_safe_name(name), "expected '{name}' to be safe");
        }
    }

    #[test]
    fn safe_name_rejects_invalid() {
        for name in ["", "MyGen", "-foo", "a/b", "a b", ".."] {
            assert!(!is_safe_name(name), "expected '{name}' to be rejected");
        }
    }

    fn valid_yaml(name: &str, scope: &str) -> String {
        format!(
            "name: {name}\nscope: {scope}\ngenerate:\n  image: img:latest\n  command: gen cmd\n"
        )
    }

    #[test]
    fn load_valid_single_def() {
        let tmp = tempfile::tempdir().unwrap();
        let custom = tmp.path().join("generators");
        fs::create_dir(&custom).unwrap();
        fs::write(custom.join("my-gen.yaml"), valid_yaml("my-gen", "server")).unwrap();

        let defs = load(tmp.path(), "generators").unwrap();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "my-gen");
        assert_eq!(defs[0].scope, "server");
    }

    #[test]
    fn load_missing_dir_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let defs = load(tmp.path(), "nope").unwrap();
        assert!(defs.is_empty());
    }

    #[test]
    fn load_builtin_collision_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let custom = tmp.path().join("generators");
        fs::create_dir(&custom).unwrap();
        fs::write(custom.join("spring.yaml"), valid_yaml("spring", "server")).unwrap();

        let err = load(tmp.path(), "generators").unwrap_err();
        assert!(err.to_string().contains("collides with built-in"));
    }

    #[test]
    fn load_duplicate_names_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let custom = tmp.path().join("generators");
        fs::create_dir(&custom).unwrap();
        fs::write(custom.join("a.yaml"), valid_yaml("my-gen", "client")).unwrap();
        fs::write(custom.join("b.yaml"), valid_yaml("my-gen", "client")).unwrap();

        let err = load(tmp.path(), "generators").unwrap_err();
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn load_invalid_scope_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let custom = tmp.path().join("generators");
        fs::create_dir(&custom).unwrap();
        fs::write(custom.join("gen.yaml"), valid_yaml("my-gen", "both")).unwrap();

        let err = load(tmp.path(), "generators").unwrap_err();
        assert!(err.to_string().contains("invalid scope"));
    }

    #[test]
    fn load_skips_non_yaml() {
        let tmp = tempfile::tempdir().unwrap();
        let custom = tmp.path().join("generators");
        fs::create_dir(&custom).unwrap();
        fs::write(custom.join("readme.txt"), "not yaml").unwrap();
        fs::write(custom.join("my-gen.yml"), valid_yaml("my-gen", "server")).unwrap();

        let defs = load(tmp.path(), "generators").unwrap();
        assert_eq!(defs.len(), 1);
    }

    #[test]
    fn same_name_different_scope_ok() {
        let tmp = tempfile::tempdir().unwrap();
        let custom = tmp.path().join("generators");
        fs::create_dir(&custom).unwrap();
        fs::write(custom.join("a.yaml"), valid_yaml("my-gen", "server")).unwrap();
        fs::write(custom.join("b.yaml"), valid_yaml("my-gen", "client")).unwrap();

        let defs = load(tmp.path(), "generators").unwrap();
        assert_eq!(defs.len(), 2);
    }

    #[test]
    fn with_compile_block() {
        let tmp = tempfile::tempdir().unwrap();
        let custom = tmp.path().join("generators");
        fs::create_dir(&custom).unwrap();
        let yaml = "\
name: my-gen
scope: server
generate:
  image: gen:latest
  command: gen --spec {spec}
compile:
  image: build:latest
  command: npm run build
";
        fs::write(custom.join("my-gen.yaml"), yaml).unwrap();

        let defs = load(tmp.path(), "generators").unwrap();
        assert!(defs[0].compile.is_some());
        assert_eq!(defs[0].compile.as_ref().unwrap().image, "build:latest");
    }

    #[test]
    fn server_and_client_name_filters() {
        let defs = vec![
            CustomGeneratorDef {
                name: "a".into(),
                scope: "server".into(),
                generate: GenerateBlock {
                    image: "i".into(),
                    command: "c".into(),
                },
                compile: None,
            },
            CustomGeneratorDef {
                name: "b".into(),
                scope: "client".into(),
                generate: GenerateBlock {
                    image: "i".into(),
                    command: "c".into(),
                },
                compile: None,
            },
        ];
        assert_eq!(server_names(&defs), vec!["a"]);
        assert_eq!(client_names(&defs), vec!["b"]);
    }
}
