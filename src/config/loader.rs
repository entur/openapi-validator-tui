use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use super::types::Config;

const CONFIG_FILE: &str = ".oavc";

/// Load config from `.oavc` in the given directory.
/// Returns the default config if the file doesn't exist.
pub fn load(root: &Path) -> Result<Config> {
    let path = root.join(CONFIG_FILE);
    if !path.exists() {
        return Ok(Config::default());
    }
    let content = fs::read_to_string(&path).context("Failed to read .oavc")?;
    let config: Config = serde_yaml::from_str(&content).context("Failed to parse .oavc")?;
    Ok(config)
}
