use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

const OAV_DIRS: &[&str] = &[
    ".oav/configs",
    ".oav/generated",
    ".oav/reports/lint",
    ".oav/reports/generate",
    ".oav/reports/compile",
];

const GITIGNORE_ENTRIES: &[&str] = &[".oav/generated/", ".oav/reports/"];

/// Create the `.oav/` directory tree under `work_dir`.
pub fn ensure_oav_dirs(work_dir: &Path) -> Result<()> {
    for dir in OAV_DIRS {
        let path = work_dir.join(dir);
        fs::create_dir_all(&path)
            .with_context(|| format!("failed to create {}", path.display()))?;
    }
    Ok(())
}

/// Append `.oav/generated/` and `.oav/reports/` to `.gitignore` if not already present.
///
/// Does nothing if `.gitignore` doesn't exist — we don't create one from scratch.
pub fn manage_gitignore(work_dir: &Path) -> Result<()> {
    let gitignore = work_dir.join(".gitignore");
    if !gitignore.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&gitignore)
        .with_context(|| format!("failed to read {}", gitignore.display()))?;

    let mut additions = Vec::new();
    for entry in GITIGNORE_ENTRIES {
        if !content.lines().any(|line| line.trim() == *entry) {
            additions.push(*entry);
        }
    }

    if additions.is_empty() {
        return Ok(());
    }

    let mut appendix = String::new();
    // Ensure we start on a new line.
    if !content.ends_with('\n') && !content.is_empty() {
        appendix.push('\n');
    }
    appendix.push_str("\n# openapi-validator-tui\n");
    for entry in &additions {
        appendix.push_str(entry);
        appendix.push('\n');
    }

    fs::write(&gitignore, format!("{content}{appendix}"))
        .with_context(|| format!("failed to write {}", gitignore.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_oav_dirs_creates_all() {
        let tmp = tempfile::tempdir().unwrap();
        ensure_oav_dirs(tmp.path()).unwrap();
        for dir in OAV_DIRS {
            assert!(tmp.path().join(dir).is_dir(), "{dir} not created");
        }
    }

    #[test]
    fn manage_gitignore_appends_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let gi = tmp.path().join(".gitignore");
        fs::write(&gi, "node_modules/\n").unwrap();

        manage_gitignore(tmp.path()).unwrap();

        let content = fs::read_to_string(&gi).unwrap();
        assert!(content.contains(".oav/generated/"));
        assert!(content.contains(".oav/reports/"));
    }

    #[test]
    fn manage_gitignore_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let gi = tmp.path().join(".gitignore");
        fs::write(&gi, ".oav/generated/\n.oav/reports/\n").unwrap();

        manage_gitignore(tmp.path()).unwrap();

        let content = fs::read_to_string(&gi).unwrap();
        // Should not duplicate entries.
        assert_eq!(content.matches(".oav/generated/").count(), 1);
        assert_eq!(content.matches(".oav/reports/").count(), 1);
    }

    #[test]
    fn manage_gitignore_skips_missing() {
        let tmp = tempfile::tempdir().unwrap();
        // No .gitignore — should return Ok without creating one.
        manage_gitignore(tmp.path()).unwrap();
        assert!(!tmp.path().join(".gitignore").exists());
    }
}
