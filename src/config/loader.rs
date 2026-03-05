use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use super::types::Config;
use crate::custom::CustomGeneratorDef;
use crate::generators;

const CONFIG_FILE: &str = ".oavc";

/// Load config from `.oavc` in the given directory.
/// Returns the default config if the file doesn't exist.
pub fn load(root: &Path) -> Result<Config> {
    let path = root.join(CONFIG_FILE);
    if !path.exists() {
        return Ok(Config::default());
    }
    if !path.is_file() {
        anyhow::bail!(".oavc exists but is not a file: {}", path.display());
    }
    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    let config: Config = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(config)
}

/// Validate config against the built-in and custom generator registries.
///
/// Returns warning messages for unknown generators. These are warnings, not
/// errors — unknown generators still run via bare `-g`.
pub fn validate(cfg: &Config, custom_defs: &[CustomGeneratorDef]) -> Vec<String> {
    let mut warnings = Vec::new();

    let is_known = |name: &str, scope: &str| -> bool {
        generators::find_builtin(name, scope).is_some()
            || custom_defs
                .iter()
                .any(|d| d.name == name && d.scope == scope)
    };

    for name in &cfg.server_generators {
        if !is_known(name, "server") {
            warnings.push(format!(
                "Unknown server generator '{name}' — no built-in or custom config available"
            ));
        }
    }

    for name in &cfg.client_generators {
        if !is_known(name, "client") {
            warnings.push(format!(
                "Unknown client generator '{name}' — no built-in or custom config available"
            ));
        }
    }

    for key in cfg.generator_config_overrides.keys() {
        let in_server = if cfg.server_generators.is_empty() {
            is_known(key, "server")
        } else {
            cfg.server_generators.iter().any(|g| g == key)
        };
        let in_client = if cfg.client_generators.is_empty() {
            is_known(key, "client")
        } else {
            cfg.client_generators.iter().any(|g| g == key)
        };
        if !in_server && !in_client {
            warnings.push(format!(
                "Config override for '{key}' but it's not in server_generators or client_generators"
            ));
        }
    }

    warnings
}
