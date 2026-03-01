use std::path::Path;

use walkdir::WalkDir;

use super::state::{CodeBrowserState, FileEntry};

/// Rebuild the file tree for the currently selected generator.
///
/// Clears existing tree state. No-ops gracefully if the generator directory
/// doesn't exist (e.g. before any pipeline run).
pub fn refresh_file_tree(state: &mut CodeBrowserState, work_dir: &Path) {
    state.file_tree.clear();
    state.file_index = 0;
    state.file_content = None;
    state.opened_file_index = None;
    state.file_scroll = 0;

    let gen_dir = match state.active_generator_dir() {
        Some(d) => d,
        None => return,
    };

    let root = work_dir.join(".generated").join(&gen_dir);
    if !root.is_dir() {
        return;
    }

    let walker = WalkDir::new(&root)
        .sort_by(|a, b| {
            let a_dir = a.file_type().is_dir();
            let b_dir = b.file_type().is_dir();
            // Dirs first, then alphabetical.
            b_dir
                .cmp(&a_dir)
                .then_with(|| a.file_name().cmp(b.file_name()))
        })
        .min_depth(1); // skip the root itself

    for entry in walker.into_iter().filter_map(|e| e.ok()) {
        let depth = entry.depth() - 1;
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = entry.file_type().is_dir();
        let path = entry.into_path();
        state.file_tree.push(FileEntry {
            depth,
            name,
            is_dir,
            path,
        });
    }
}

/// Load the file at the current `file_index` into `file_content`.
///
/// Skips directories and symlinks. Detects binary files (null bytes in first 8KB).
/// Truncates files larger than 512KB with a notice.
pub fn load_selected_file(state: &mut CodeBrowserState) {
    let entry = match state.file_tree.get(state.file_index) {
        Some(e) => e,
        None => return,
    };

    if entry.is_dir {
        return;
    }

    let path = &entry.path;

    const BINARY_PROBE: usize = 8192;
    const MAX_SIZE: usize = 512 * 1024;

    // Use symlink_metadata to avoid following symlinks outside .generated/.
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(_) => {
            state.file_content = Some(vec!["[Cannot read file]".into()]);
            state.opened_file_index = Some(state.file_index);
            state.content_version += 1;
            state.file_scroll = 0;
            return;
        }
    };

    if !metadata.is_file() {
        state.file_content = Some(vec!["[Not a regular file]".into()]);
        state.opened_file_index = Some(state.file_index);
        state.content_version += 1;
        state.file_scroll = 0;
        return;
    }

    // Read only what we need via a File handle.
    use std::io::Read;
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => {
            state.file_content = Some(vec!["[Cannot read file]".into()]);
            state.opened_file_index = Some(state.file_index);
            state.content_version += 1;
            state.file_scroll = 0;
            return;
        }
    };

    // Binary detection: read first 8KB and check for null bytes.
    let mut probe = vec![0u8; BINARY_PROBE];
    let probe_len = match file.read(&mut probe) {
        Ok(n) => n,
        Err(_) => {
            state.file_content = Some(vec!["[Cannot read file]".into()]);
            state.opened_file_index = Some(state.file_index);
            state.content_version += 1;
            state.file_scroll = 0;
            return;
        }
    };

    if probe[..probe_len].contains(&0) {
        state.file_content = Some(vec!["[Binary file — cannot display]".into()]);
        state.opened_file_index = Some(state.file_index);
        state.content_version += 1;
        state.file_scroll = 0;
        return;
    }

    // Read up to MAX_SIZE total (we already have probe_len bytes).
    let remaining = MAX_SIZE.saturating_sub(probe_len);
    let mut rest = vec![0u8; remaining];
    let rest_len = file.read(&mut rest).unwrap_or(0);

    let total = probe_len + rest_len;
    let mut bytes = probe;
    bytes.truncate(probe_len);
    bytes.extend_from_slice(&rest[..rest_len]);

    let text = String::from_utf8_lossy(&bytes);
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();

    let file_size = metadata.len() as usize;
    if file_size > total {
        lines.push(String::new());
        lines.push(format!(
            "[File truncated — showing first {}KB of {}KB]",
            total / 1024,
            file_size / 1024
        ));
    }

    state.file_content = Some(lines);
    state.opened_file_index = Some(state.file_index);
    state.content_version += 1;
    state.file_scroll = 0;
}

/// Map a file extension to the syntect syntax name.
pub fn syntax_name_for_path(path: &Path) -> &'static str {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    match ext.to_ascii_lowercase().as_str() {
        "java" => "Java",
        "kt" | "kts" => "Kotlin",
        "ts" | "tsx" => "TypeScript",
        "js" | "jsx" | "mjs" | "cjs" => "JavaScript",
        "go" => "Go",
        "py" | "pyi" => "Python",
        "rs" => "Rust",
        "cs" => "C#",
        "rb" => "Ruby",
        "swift" => "Swift",
        "c" | "h" => "C",
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => "C++",
        "json" => "JSON",
        "yaml" | "yml" => "YAML",
        "xml" | "xsd" | "wsdl" => "XML",
        "html" | "htm" => "HTML",
        "css" => "CSS",
        "md" | "markdown" => "Markdown",
        "sh" | "bash" | "zsh" => "Bourne Again Shell (bash)",
        "toml" => "TOML",
        "sql" => "SQL",
        "gradle" => "Groovy",
        "dart" => "Dart",
        "php" => "PHP",
        "scala" => "Scala",
        _ => "Plain Text",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_state() -> CodeBrowserState {
        CodeBrowserState::new()
    }

    // ── refresh_file_tree ────────────────────────────────────────────

    #[test]
    fn refresh_nonexistent_dir_gives_empty_tree() {
        let mut state = make_state();
        state.generators = vec![("go".into(), "server".into())];
        refresh_file_tree(&mut state, Path::new("/tmp/no_such_dir_12345"));
        assert!(state.file_tree.is_empty());
    }

    #[test]
    fn refresh_populated_dir_builds_tree() {
        let tmp = TempDir::new().unwrap();
        let gen_dir = tmp.path().join(".generated/go-server");
        std::fs::create_dir_all(gen_dir.join("src/main")).unwrap();
        std::fs::write(gen_dir.join("build.gradle"), "apply plugin").unwrap();
        std::fs::write(gen_dir.join("src/main/App.java"), "class App {}").unwrap();

        let mut state = make_state();
        state.generators = vec![("go".into(), "server".into())];
        refresh_file_tree(&mut state, tmp.path());

        assert!(!state.file_tree.is_empty());

        // Dirs should come before files at same level.
        let names: Vec<&str> = state.file_tree.iter().map(|e| e.name.as_str()).collect();
        let src_pos = names.iter().position(|n| *n == "src").unwrap();
        let gradle_pos = names.iter().position(|n| *n == "build.gradle").unwrap();
        assert!(src_pos < gradle_pos, "dirs should sort before files");

        // Check depth for nested file.
        let app_entry = state
            .file_tree
            .iter()
            .find(|e| e.name == "App.java")
            .unwrap();
        assert_eq!(app_entry.depth, 2); // src/main/App.java
        assert!(!app_entry.is_dir);
    }

    #[test]
    fn refresh_clears_previous_state() {
        let mut state = make_state();
        state.generators = vec![("go".into(), "server".into())];
        state.file_index = 5;
        state.file_content = Some(vec!["old".into()]);
        state.file_scroll = 10;

        refresh_file_tree(&mut state, Path::new("/tmp/no_such_dir_12345"));

        assert_eq!(state.file_index, 0);
        assert!(state.file_content.is_none());
        assert_eq!(state.file_scroll, 0);
    }

    #[test]
    fn refresh_no_generators_noops() {
        let mut state = make_state();
        // No generators set
        refresh_file_tree(&mut state, Path::new("/tmp"));
        assert!(state.file_tree.is_empty());
    }

    // ── load_selected_file ───────────────────────────────────────────

    #[test]
    fn load_dir_entry_does_not_set_content() {
        let mut state = make_state();
        state.file_tree.push(super::FileEntry {
            depth: 0,
            name: "src".into(),
            is_dir: true,
            path: PathBuf::from("/tmp"),
        });
        state.file_index = 0;

        let version_before = state.content_version;
        load_selected_file(&mut state);
        assert!(state.file_content.is_none());
        assert_eq!(state.content_version, version_before);
    }

    #[test]
    fn load_text_file_sets_content_and_increments_version() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.txt");
        std::fs::write(&path, "line one\nline two\nline three").unwrap();

        let mut state = make_state();
        state.file_tree.push(super::FileEntry {
            depth: 0,
            name: "test.txt".into(),
            is_dir: false,
            path: path.clone(),
        });
        state.file_index = 0;
        let version_before = state.content_version;

        load_selected_file(&mut state);

        assert!(state.file_content.is_some());
        let lines = state.file_content.as_ref().unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "line one");
        assert_eq!(state.content_version, version_before + 1);
        assert_eq!(state.file_scroll, 0);
    }

    #[test]
    fn load_binary_file_shows_message() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("image.bin");
        let mut data = vec![0x89, 0x50, 0x4E, 0x47]; // PNG header
        data.push(0x00); // null byte
        data.extend_from_slice(&[0xFF; 100]);
        std::fs::write(&path, &data).unwrap();

        let mut state = make_state();
        state.file_tree.push(super::FileEntry {
            depth: 0,
            name: "image.bin".into(),
            is_dir: false,
            path: path.clone(),
        });
        state.file_index = 0;

        load_selected_file(&mut state);

        let lines = state.file_content.as_ref().unwrap();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("Binary file"));
    }

    // ── syntax_name_for_path ─────────────────────────────────────────

    #[test]
    fn syntax_name_known_extensions() {
        assert_eq!(syntax_name_for_path(Path::new("App.java")), "Java");
        assert_eq!(syntax_name_for_path(Path::new("index.ts")), "TypeScript");
        assert_eq!(syntax_name_for_path(Path::new("main.go")), "Go");
        assert_eq!(syntax_name_for_path(Path::new("lib.py")), "Python");
        assert_eq!(syntax_name_for_path(Path::new("Foo.cs")), "C#");
        assert_eq!(syntax_name_for_path(Path::new("lib.rs")), "Rust");
    }

    #[test]
    fn syntax_name_unknown_falls_back() {
        assert_eq!(syntax_name_for_path(Path::new("README")), "Plain Text");
        assert_eq!(syntax_name_for_path(Path::new("file.xyz")), "Plain Text");
    }
}
