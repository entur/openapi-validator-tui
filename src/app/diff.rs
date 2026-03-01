use std::collections::HashMap;
use std::path::{Path, PathBuf};

use similar::TextDiff;
use walkdir::WalkDir;

// ── Types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    Context(String),
    Insert(String),
    Delete(String),
    HunkHeader(String),
}

#[derive(Debug, Clone)]
pub struct FileDiff {
    pub rel_path: String,
    pub kind: ChangeKind,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone)]
pub struct GeneratorDiff {
    pub generator: String,
    pub scope: String,
    pub files: Vec<FileDiff>,
}

/// Which sub-panel has focus within the diff view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffPanel {
    FileList,
    DiffContent,
}

pub struct DiffViewState {
    pub diffs: HashMap<String, GeneratorDiff>,
    pub active: bool,
    pub file_index: usize,
    pub scroll: u16,
    pub focus: DiffPanel,
    pub active_generator: Option<String>,
}

impl DiffViewState {
    pub fn new() -> Self {
        Self {
            diffs: HashMap::new(),
            active: false,
            file_index: 0,
            scroll: 0,
            focus: DiffPanel::FileList,
            active_generator: None,
        }
    }

    pub fn reset_nav(&mut self) {
        self.file_index = 0;
        self.scroll = 0;
        self.focus = DiffPanel::FileList;
    }

    pub fn active_diff(&self) -> Option<&GeneratorDiff> {
        self.active_generator
            .as_ref()
            .and_then(|key| self.diffs.get(key))
    }

    #[cfg(test)]
    pub fn total_changed_files(&self) -> usize {
        self.diffs.values().map(|d| d.files.len()).sum()
    }
}

// ── Snapshot ─────────────────────────────────────────────────────────

const MAX_FILE_SIZE: u64 = 512 * 1024;
const BINARY_PROBE_SIZE: usize = 8192;

/// Walk `root` and return a map of relative paths → file contents.
/// Skips binary files and files larger than `MAX_FILE_SIZE`.
pub fn snapshot_directory(root: &Path) -> HashMap<PathBuf, String> {
    let mut snapshot = HashMap::new();

    if !root.is_dir() {
        return snapshot;
    }

    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }

        if let Ok(meta) = entry.metadata()
            && meta.len() > MAX_FILE_SIZE
        {
            continue;
        }

        let Ok(content) = std::fs::read(entry.path()) else {
            continue;
        };

        let probe = &content[..content.len().min(BINARY_PROBE_SIZE)];
        if probe.contains(&0u8) {
            continue;
        }

        let Ok(text) = String::from_utf8(content) else {
            continue;
        };

        let rel = entry
            .path()
            .strip_prefix(root)
            .unwrap_or(entry.path())
            .to_path_buf();

        snapshot.insert(rel, text);
    }

    snapshot
}

// ── Diff computation ─────────────────────────────────────────────────

pub fn compute_diff(
    generator: &str,
    scope: &str,
    before: &HashMap<PathBuf, String>,
    gen_root: &Path,
) -> GeneratorDiff {
    let after = snapshot_directory(gen_root);
    let mut files = Vec::new();

    for (rel, before_text) in before {
        if !after.contains_key(rel) {
            files.push(FileDiff {
                rel_path: rel.to_string_lossy().into_owned(),
                kind: ChangeKind::Deleted,
                lines: make_delete_lines(before_text),
            });
        }
    }

    for (rel, after_text) in &after {
        match before.get(rel) {
            None => {
                files.push(FileDiff {
                    rel_path: rel.to_string_lossy().into_owned(),
                    kind: ChangeKind::Added,
                    lines: make_add_lines(after_text),
                });
            }
            Some(before_text) if before_text != after_text => {
                files.push(FileDiff {
                    rel_path: rel.to_string_lossy().into_owned(),
                    kind: ChangeKind::Modified,
                    lines: make_unified_diff(before_text, after_text),
                });
            }
            _ => {}
        }
    }

    files.sort_unstable_by(|a, b| a.rel_path.cmp(&b.rel_path));

    GeneratorDiff {
        generator: generator.into(),
        scope: scope.into(),
        files,
    }
}

fn make_unified_diff(old: &str, new: &str) -> Vec<DiffLine> {
    let text_diff = TextDiff::from_lines(old, new);
    let mut lines = Vec::new();

    for hunk in text_diff.unified_diff().context_radius(3).iter_hunks() {
        lines.push(DiffLine::HunkHeader(hunk.header().to_string()));
        for change in hunk.iter_changes() {
            let text = change.value().trim_end_matches('\n').to_string();
            match change.tag() {
                similar::ChangeTag::Equal => lines.push(DiffLine::Context(text)),
                similar::ChangeTag::Insert => lines.push(DiffLine::Insert(text)),
                similar::ChangeTag::Delete => lines.push(DiffLine::Delete(text)),
            }
        }
    }

    lines
}

fn make_add_lines(content: &str) -> Vec<DiffLine> {
    let mut lines = Vec::new();
    lines.push(DiffLine::HunkHeader("@@ new file @@".into()));
    for line in content.lines() {
        lines.push(DiffLine::Insert(line.to_string()));
    }
    lines
}

fn make_delete_lines(content: &str) -> Vec<DiffLine> {
    let mut lines = Vec::new();
    lines.push(DiffLine::HunkHeader("@@ deleted file @@".into()));
    for line in content.lines() {
        lines.push(DiffLine::Delete(line.to_string()));
    }
    lines
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn snapshot_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let snap = snapshot_directory(dir.path());
        assert!(snap.is_empty());
    }

    #[test]
    fn snapshot_text_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "hello").unwrap();
        fs::create_dir_all(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub/b.txt"), "world").unwrap();

        let snap = snapshot_directory(dir.path());
        assert_eq!(snap.len(), 2);
        assert_eq!(snap[&PathBuf::from("a.txt")], "hello");
        assert_eq!(snap[&PathBuf::from("sub/b.txt")], "world");
    }

    #[test]
    fn snapshot_skips_binary_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("text.txt"), "ok").unwrap();
        fs::write(dir.path().join("bin.dat"), b"ab\x00cd").unwrap();

        let snap = snapshot_directory(dir.path());
        assert_eq!(snap.len(), 1);
        assert!(snap.contains_key(&PathBuf::from("text.txt")));
    }

    #[test]
    fn snapshot_nonexistent_dir() {
        let snap = snapshot_directory(Path::new("/nonexistent/path/xyz"));
        assert!(snap.is_empty());
    }

    #[test]
    fn compute_diff_all_added() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("new.txt"), "line1\nline2\n").unwrap();

        let before = HashMap::new();
        let diff = compute_diff("go", "server", &before, dir.path());

        assert_eq!(diff.generator, "go");
        assert_eq!(diff.scope, "server");
        assert_eq!(diff.files.len(), 1);
        assert_eq!(diff.files[0].kind, ChangeKind::Added);
        assert_eq!(diff.files[0].rel_path, "new.txt");
        assert!(
            diff.files[0]
                .lines
                .iter()
                .any(|l| matches!(l, DiffLine::Insert(..)))
        );
    }

    #[test]
    fn compute_diff_all_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let mut before = HashMap::new();
        before.insert(PathBuf::from("old.txt"), "deleted content\n".into());

        let diff = compute_diff("ts", "client", &before, dir.path());
        assert_eq!(diff.files.len(), 1);
        assert_eq!(diff.files[0].kind, ChangeKind::Deleted);
        assert!(
            diff.files[0]
                .lines
                .iter()
                .any(|l| matches!(l, DiffLine::Delete(..)))
        );
    }

    #[test]
    fn compute_diff_modified() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("file.txt"), "line1\nline2 changed\nline3\n").unwrap();

        let mut before = HashMap::new();
        before.insert(PathBuf::from("file.txt"), "line1\nline2\nline3\n".into());

        let diff = compute_diff("go", "server", &before, dir.path());
        assert_eq!(diff.files.len(), 1);
        assert_eq!(diff.files[0].kind, ChangeKind::Modified);
        let has_insert = diff.files[0]
            .lines
            .iter()
            .any(|l| matches!(l, DiffLine::Insert(..)));
        let has_delete = diff.files[0]
            .lines
            .iter()
            .any(|l| matches!(l, DiffLine::Delete(..)));
        assert!(has_insert);
        assert!(has_delete);
    }

    #[test]
    fn compute_diff_unchanged_excluded() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("same.txt"), "unchanged\n").unwrap();

        let mut before = HashMap::new();
        before.insert(PathBuf::from("same.txt"), "unchanged\n".into());

        let diff = compute_diff("go", "server", &before, dir.path());
        assert!(diff.files.is_empty());
    }

    #[test]
    fn compute_diff_empty_snapshot_first_run() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.java"), "class A {}").unwrap();
        fs::write(dir.path().join("b.java"), "class B {}").unwrap();

        let before = HashMap::new();
        let diff = compute_diff("java", "server", &before, dir.path());
        assert_eq!(diff.files.len(), 2);
        assert!(diff.files.iter().all(|f| f.kind == ChangeKind::Added));
    }

    #[test]
    fn diff_view_state_total_changed_files() {
        let mut state = DiffViewState::new();
        state.diffs.insert(
            "go-server".into(),
            GeneratorDiff {
                generator: "go".into(),
                scope: "server".into(),
                files: vec![FileDiff {
                    rel_path: "a.go".into(),
                    kind: ChangeKind::Added,
                    lines: vec![],
                }],
            },
        );
        state.diffs.insert(
            "ts-client".into(),
            GeneratorDiff {
                generator: "ts".into(),
                scope: "client".into(),
                files: vec![
                    FileDiff {
                        rel_path: "b.ts".into(),
                        kind: ChangeKind::Modified,
                        lines: vec![],
                    },
                    FileDiff {
                        rel_path: "c.ts".into(),
                        kind: ChangeKind::Deleted,
                        lines: vec![],
                    },
                ],
            },
        );
        assert_eq!(state.total_changed_files(), 3);
    }
}
