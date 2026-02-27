use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use walkdir::WalkDir;

/// Resolve a spec path (from config or CLI) to a relative path within the project root.
pub fn normalize_spec_path(root: &Path, spec: &str) -> Result<PathBuf> {
    if spec.trim().is_empty() {
        bail!("Spec path cannot be blank");
    }
    let spec_path = PathBuf::from(spec);
    let absolute = if spec_path.is_absolute() {
        spec_path
    } else {
        root.join(&spec_path)
    };
    if !absolute.exists() {
        bail!("Spec file not found: {}", absolute.display());
    }
    let relative = absolute
        .strip_prefix(root)
        .context("Spec path must be inside the project root")?;
    Ok(relative.to_path_buf())
}

/// Walk the directory tree to find OpenAPI spec files.
/// Returns a sorted list of relative paths.
pub fn discover_spec(root: &Path, max_depth: usize) -> Result<Vec<String>> {
    // Check well-known names first.
    for name in ["openapi.yaml", "openapi.yml"] {
        if root.join(name).is_file() {
            return Ok(vec![name.to_string()]);
        }
    }

    let mut matches = Vec::new();
    let walker = WalkDir::new(root)
        .max_depth(max_depth)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !should_skip(e));

    for entry in walker.filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if is_yaml(path) && is_openapi_spec(path) {
            if let Ok(rel) = path.strip_prefix(root) {
                matches.push(rel.to_string_lossy().to_string());
            }
        }
    }

    matches.sort();
    Ok(matches)
}

fn is_yaml(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("yaml" | "yml" | "YAML" | "YML")
    )
}

fn is_openapi_spec(path: &Path) -> bool {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut content = String::new();
    if file.read_to_string(&mut content).is_err() {
        return false;
    }
    let doc: serde_yaml::Value = match serde_yaml::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };
    match doc {
        serde_yaml::Value::Mapping(mapping) => mapping
            .keys()
            .filter_map(|k| k.as_str())
            .any(|k| k == "openapi"),
        _ => false,
    }
}

fn should_skip(entry: &walkdir::DirEntry) -> bool {
    if entry.depth() == 0 || !entry.file_type().is_dir() {
        return false;
    }
    matches!(
        entry.file_name().to_str().unwrap_or_default(),
        ".git" | ".oav" | "target" | "node_modules" | ".idea" | ".vscode"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn discover_finds_well_known_name() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("openapi.yaml"),
            "openapi: 3.0.0\ninfo:\n  title: Test\n  version: '1.0'\npaths: {}\n",
        )
        .unwrap();

        let specs = discover_spec(dir.path(), 4).unwrap();
        assert_eq!(specs, vec!["openapi.yaml"]);
    }

    #[test]
    fn discover_finds_nested_spec() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("api");
        fs::create_dir_all(&sub).unwrap();
        fs::write(
            sub.join("spec.yml"),
            "openapi: 3.1.0\ninfo:\n  title: Nested\n  version: '1.0'\npaths: {}\n",
        )
        .unwrap();

        let specs = discover_spec(dir.path(), 4).unwrap();
        assert_eq!(specs, vec!["api/spec.yml"]);
    }

    #[test]
    fn discover_ignores_non_openapi_yaml() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("config.yaml"),
            "database:\n  host: localhost\n",
        )
        .unwrap();

        let specs = discover_spec(dir.path(), 4).unwrap();
        assert!(specs.is_empty());
    }

    #[test]
    fn discover_skips_ignored_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let hidden = dir.path().join("node_modules").join("some-package");
        fs::create_dir_all(&hidden).unwrap();
        fs::write(
            hidden.join("openapi.yaml"),
            "openapi: 3.0.0\ninfo:\n  title: Hidden\n  version: '1.0'\npaths: {}\n",
        )
        .unwrap();

        let specs = discover_spec(dir.path(), 4).unwrap();
        assert!(specs.is_empty());
    }

    #[test]
    fn normalize_rejects_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let result = normalize_spec_path(dir.path(), "nonexistent.yaml");
        assert!(result.is_err());
    }

    #[test]
    fn normalize_resolves_relative_path() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("api.yaml"), "openapi: 3.0.0\n").unwrap();

        let path = normalize_spec_path(dir.path(), "api.yaml").unwrap();
        assert_eq!(path, PathBuf::from("api.yaml"));
    }
}
