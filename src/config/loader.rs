use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use super::types::Config;
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

/// Validate config against the built-in generator registry.
///
/// Returns warning messages for unknown generators. These are warnings, not
/// errors — unknown generators still run via bare `-g`.
pub fn validate(cfg: &Config) -> Vec<String> {
    let mut warnings = Vec::new();

    for name in &cfg.server_generators {
        if generators::find_builtin(name, "server").is_none() {
            warnings.push(format!(
                "Unknown server generator '{name}' — no built-in config available"
            ));
        }
    }

    for name in &cfg.client_generators {
        if generators::find_builtin(name, "client").is_none() {
            warnings.push(format!(
                "Unknown client generator '{name}' — no built-in config available"
            ));
        }
    }

    for key in cfg.generator_config_overrides.keys() {
        // When a generator list is empty it defaults to all builtins, so an
        // override for a known builtin in that scope is valid.
        let in_server = if cfg.server_generators.is_empty() {
            generators::find_builtin(key, "server").is_some()
        } else {
            cfg.server_generators.iter().any(|g| g == key)
        };
        let in_client = if cfg.client_generators.is_empty() {
            generators::find_builtin(key, "client").is_some()
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
