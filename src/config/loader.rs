use serde::Deserialize;

/// Subset of the oav config that the TUI needs.
#[derive(Debug, Deserialize, Default)]
pub struct Config {
    pub spec: Option<String>,
    pub mode: Option<String>,
    pub lint: Option<bool>,
    pub generate: Option<bool>,
    pub compile: Option<bool>,
    pub linter: Option<String>,
    pub server_generators: Option<Vec<String>>,
    pub client_generators: Option<Vec<String>>,
    pub docker_timeout: Option<u64>,
    pub jobs: Option<usize>,
}

impl Config {
    /// Load config from a `.oavc` file in the given directory.
    pub fn load(dir: &std::path::Path) -> anyhow::Result<Option<Self>> {
        let path = dir.join(".oavc");
        if !path.exists() {
            return Ok(None);
        }
        let contents = std::fs::read_to_string(&path)?;
        let config: Config = serde_yaml::from_str(&contents)?;
        Ok(Some(config))
    }
}
