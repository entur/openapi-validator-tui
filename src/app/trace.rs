use std::collections::HashMap;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

/// A single error parsed from a compile log.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CompileError {
    /// Path relative to the generated output root.
    pub file: PathBuf,
    pub line: usize,
    pub col: Option<usize>,
    pub message: String,
}

/// Bidirectional mapping between spec constructs and generated files.
#[derive(Debug, Clone, Default)]
pub struct TraceIndex {
    /// Spec construct name (e.g. "Pet", "listPets") → generated file relative paths.
    pub spec_to_files: HashMap<String, Vec<PathBuf>>,
    /// Generated file relative path → likely spec construct names.
    pub file_to_spec: HashMap<PathBuf, Vec<String>>,
}

// ── Compile error parsing ──────────────────────────────────────────────

/// Parse compile log output into structured errors.
///
/// Recognizes patterns for Java/Kotlin, TypeScript, Go, Python, and C#.
/// Container paths (`/work/.oav/generated/...`) are stripped to relative paths.
pub fn parse_compile_errors(log: &str) -> Vec<CompileError> {
    let mut errors = Vec::new();
    for line in log.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(err) = try_java_kotlin(trimmed)
            .or_else(|| try_typescript(trimmed))
            .or_else(|| try_go(trimmed))
            .or_else(|| try_python(trimmed))
            .or_else(|| try_csharp(trimmed))
        {
            errors.push(err);
        }
    }
    errors
}

/// Java/Kotlin: `path/File.java:42: error: message`
fn try_java_kotlin(line: &str) -> Option<CompileError> {
    // Must contain ": error:" or ": warning:" to be a real compile line.
    let error_marker = line.find(": error:").or_else(|| line.find(": warning:"))?;

    // Find the file:line portion before the marker.
    let prefix = &line[..error_marker];
    let colon = prefix.rfind(':')?;
    let file_str = &prefix[..colon];
    let line_str = &prefix[colon + 1..];
    let line_num: usize = line_str.parse().ok()?;

    // Require it looks like a source file.
    if !file_str.ends_with(".java")
        && !file_str.ends_with(".kt")
        && !file_str.ends_with(".kts")
        && !file_str.ends_with(".scala")
    {
        return None;
    }

    let message = line[error_marker + 2..].trim().to_string();

    Some(CompileError {
        file: normalize_container_path(file_str),
        line: line_num,
        col: None,
        message,
    })
}

/// TypeScript: `path/file.ts(42,10): error TS2345: message`
/// Also: C#-style `path/File.cs(42,10): error CS0001: message`
fn try_typescript(line: &str) -> Option<CompileError> {
    let paren = line.find('(')?;
    let close_paren = line[paren..].find(')')? + paren;
    let file_str = &line[..paren];

    if !file_str.ends_with(".ts")
        && !file_str.ends_with(".tsx")
        && !file_str.ends_with(".js")
        && !file_str.ends_with(".mts")
    {
        return None;
    }

    let loc = &line[paren + 1..close_paren];
    let (line_num, col) = parse_comma_loc(loc)?;

    let rest = line[close_paren + 1..].trim();
    let message = rest.strip_prefix(':').unwrap_or(rest).trim().to_string();

    Some(CompileError {
        file: normalize_container_path(file_str),
        line: line_num,
        col: Some(col),
        message,
    })
}

/// Go: `path/file.go:42:10: message`
fn try_go(line: &str) -> Option<CompileError> {
    // Pattern: file.go:line:col: message
    let parts: Vec<&str> = line.splitn(4, ':').collect();
    if parts.len() < 3 {
        return None;
    }

    let file_str = parts[0];
    if !file_str.ends_with(".go") {
        return None;
    }

    let line_num: usize = parts[1].trim().parse().ok()?;
    let col: usize = parts[2].trim().parse().ok().unwrap_or(0);
    let message = if parts.len() > 3 {
        parts[3].trim().to_string()
    } else {
        String::new()
    };

    Some(CompileError {
        file: normalize_container_path(file_str),
        line: line_num,
        col: if col > 0 { Some(col) } else { None },
        message,
    })
}

/// Python: `  File "path/file.py", line 42`
fn try_python(line: &str) -> Option<CompileError> {
    let trimmed = line.trim();
    if !trimmed.starts_with("File \"") {
        return None;
    }
    let after_quote = &trimmed[6..]; // skip 'File "'
    let end_quote = after_quote.find('"')?;
    let file_str = &after_quote[..end_quote];

    if !file_str.ends_with(".py") && !file_str.ends_with(".pyi") {
        return None;
    }

    let rest = &after_quote[end_quote + 1..];
    let line_marker = rest.find("line ")?;
    let line_start = line_marker + 5;
    let line_end = rest[line_start..]
        .find(|c: char| !c.is_ascii_digit())
        .map(|i| line_start + i)
        .unwrap_or(rest.len());
    let line_num: usize = rest[line_start..line_end].parse().ok()?;

    Some(CompileError {
        file: normalize_container_path(file_str),
        line: line_num,
        col: None,
        message: String::new(),
    })
}

/// C#: `path/File.cs(42,10): error CS0001: message`
fn try_csharp(line: &str) -> Option<CompileError> {
    let paren = line.find('(')?;
    let close_paren = line[paren..].find(')')? + paren;
    let file_str = &line[..paren];

    if !file_str.ends_with(".cs") {
        return None;
    }

    let loc = &line[paren + 1..close_paren];
    let (line_num, col) = parse_comma_loc(loc)?;

    let rest = line[close_paren + 1..].trim();
    let message = rest.strip_prefix(':').unwrap_or(rest).trim().to_string();

    Some(CompileError {
        file: normalize_container_path(file_str),
        line: line_num,
        col: Some(col),
        message,
    })
}

fn parse_comma_loc(loc: &str) -> Option<(usize, usize)> {
    let (l, c) = loc.split_once(',')?;
    Some((l.trim().parse().ok()?, c.trim().parse().ok()?))
}

/// Strip container path prefixes so we get a path relative to the generated root.
///
/// Docker mounts at `/work`, so paths like `/work/.oav/generated/server/spring/src/Foo.java`
/// become `src/Foo.java` (relative to the generator output directory).
fn normalize_container_path(raw: &str) -> PathBuf {
    let path = raw.trim();

    // Strip `/work/.oav/generated/{scope}/{generator}/` prefix.
    if let Some(idx) = path.find(".oav/generated/") {
        let after = &path[idx + ".oav/generated/".len()..];
        // Skip the next two path segments (scope/generator).
        let rest = after.splitn(3, '/').nth(2).unwrap_or(after);
        return PathBuf::from(rest);
    }

    // Strip `/work/` prefix if present.
    if let Some(rest) = path.strip_prefix("/work/") {
        return PathBuf::from(rest);
    }

    PathBuf::from(path)
}

// ── Spec ↔ codegen mapping ─────────────────────────────────────────────

/// Build a trace index by scanning generated files for spec construct references.
///
/// Uses multiple heuristics:
/// 1. Name-based: model file names → schema names, api file names → operation groups
/// 2. Comment-based: scan for `@Schema`, JSON pointers, operationId in generated code
/// 3. Structure-based: `model/` dirs → schemas, `api/` dirs → operations
pub fn build_trace_index(generated_dir: &Path, spec_names: &SpecNames) -> TraceIndex {
    let mut index = TraceIndex::default();

    if !generated_dir.is_dir() {
        return index;
    }

    let walker = WalkDir::new(generated_dir)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file());

    for entry in walker {
        let abs_path = entry.path();
        let rel_path = abs_path
            .strip_prefix(generated_dir)
            .unwrap_or(abs_path)
            .to_path_buf();

        let file_stem = abs_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

        // Strategy 1: Name-based matching against schema and operation names.
        let matched_names = name_based_match(file_stem, &rel_path, spec_names);

        // Strategy 2: Comment-based — scan file content for spec references.
        let content_matches = if is_source_file(abs_path) {
            comment_based_match(abs_path, spec_names)
        } else {
            Vec::new()
        };

        let mut all_matches: Vec<String> = matched_names;
        for m in content_matches {
            if !all_matches.contains(&m) {
                all_matches.push(m);
            }
        }

        if !all_matches.is_empty() {
            for name in &all_matches {
                index
                    .spec_to_files
                    .entry(name.clone())
                    .or_default()
                    .push(rel_path.clone());
            }
            index.file_to_spec.insert(rel_path, all_matches);
        }
    }

    index
}

/// Known spec construct names extracted from a SpecIndex.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct SpecNames {
    /// Schema names (e.g. "Pet", "Order").
    pub schemas: Vec<String>,
    /// Operation IDs (e.g. "listPets", "createOrder").
    pub operation_ids: Vec<String>,
    /// Tag names.
    pub tags: Vec<String>,
    /// API path segments (e.g. "/pets", "/orders/{orderId}").
    /// Stored as decoded path segments (with `~1` → `/` unescaped).
    pub api_paths: Vec<String>,
}

impl SpecNames {
    /// Extract spec construct names from the SpecIndex's JSON pointers.
    pub fn from_pointers(pointers: &[String]) -> Self {
        let mut names = Self::default();

        for pointer in pointers {
            let parts: Vec<&str> = pointer.split('/').collect();

            // /components/schemas/{Name}
            if parts.len() >= 4 && parts[1] == "components" && parts[2] == "schemas" {
                let name = parts[3].to_string();
                if !names.schemas.contains(&name) {
                    names.schemas.push(name);
                }
            }

            // /paths/{path}/{method} — extract the API path and derive resource names.
            if parts.len() >= 3 && parts[1] == "paths" {
                let raw_path = parts[2].replace("~1", "/").replace("~0", "~");
                if !names.api_paths.contains(&raw_path) {
                    names.api_paths.push(raw_path);
                }
            }
        }

        names
    }

    /// Derive searchable resource names from API paths.
    ///
    /// `/pets` → `"pets"`, `/pets/{petId}` → `"pets"`,
    /// `/users/{userId}/orders` → `"orders"`, `"users"`.
    pub fn path_resource_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for path in &self.api_paths {
            for segment in path.split('/') {
                if segment.is_empty() || segment.starts_with('{') {
                    continue;
                }
                let lower = segment.to_ascii_lowercase();
                if !names.contains(&lower) {
                    names.push(lower);
                }
            }
        }
        names
    }
}

/// Name-based heuristic: match file stem and path against known spec names.
fn name_based_match(file_stem: &str, rel_path: &Path, names: &SpecNames) -> Vec<String> {
    let mut matches = Vec::new();
    let stem_lower = file_stem.to_ascii_lowercase();
    let path_str = rel_path.to_string_lossy().to_ascii_lowercase();

    // Check if path is in a model-like directory.
    let is_model_dir = path_str.contains("/model/")
        || path_str.contains("/models/")
        || path_str.contains("/dto/")
        || path_str.contains("/entity/")
        || path_str.contains("/schema/")
        || path_str.contains("/schemas/");

    // Check if path is in an api-like directory.
    let is_api_dir = path_str.contains("/api/")
        || path_str.contains("/apis/")
        || path_str.contains("/controller/")
        || path_str.contains("/controllers/")
        || path_str.contains("/resource/")
        || path_str.contains("/resources/");

    for schema in &names.schemas {
        let schema_lower = schema.to_ascii_lowercase();

        // Direct match: Pet.java → Pet
        if stem_lower == schema_lower {
            matches.push(schema.clone());
            continue;
        }

        // Suffixed match: PetDto.java, PetModel.java, PetResponse.java
        if stem_lower.starts_with(&schema_lower)
            && (is_model_dir || stem_lower.len() <= schema_lower.len() + 10)
        {
            let suffix = &stem_lower[schema_lower.len()..];
            if matches!(
                suffix,
                "dto" | "model" | "entity" | "response" | "request" | "schema" | "vo" | "bean"
            ) {
                matches.push(schema.clone());
            }
        }
    }

    for op_id in &names.operation_ids {
        let op_lower = op_id.to_ascii_lowercase();

        // Direct or suffixed match in api directories.
        if is_api_dir && (stem_lower == op_lower || stem_lower.contains(&op_lower)) {
            matches.push(op_id.clone());
        }
    }

    // Match API path resource names against files in api-like directories.
    // e.g., /pets → PetsApi.java, pets_controller.py, PetsService.ts
    let resource_names = names.path_resource_names();
    for resource in &resource_names {
        if stem_lower.contains(resource) && (is_api_dir || stem_lower.ends_with("api")) {
            // Store the original API path for trace-back.
            let api_path = names
                .api_paths
                .iter()
                .find(|p| {
                    p.split('/')
                        .any(|seg| seg.to_ascii_lowercase() == *resource)
                })
                .cloned()
                .unwrap_or_else(|| resource.clone());
            if !matches.contains(&api_path) {
                matches.push(api_path);
            }
        }
    }

    matches
}

/// Check if a file is a source file worth scanning for comments.
fn is_source_file(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "java"
            | "kt"
            | "kts"
            | "ts"
            | "tsx"
            | "js"
            | "go"
            | "py"
            | "cs"
            | "scala"
            | "swift"
            | "rs"
            | "rb"
            | "dart"
            | "php"
    )
}

/// Scan file content for spec references in comments/annotations.
fn comment_based_match(path: &Path, names: &SpecNames) -> Vec<String> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    // Cap scan size to avoid reading huge files.
    let scan = if content.len() > 256 * 1024 {
        &content[..256 * 1024]
    } else {
        &content
    };

    let mut matches = Vec::new();

    for schema in &names.schemas {
        // @Schema(name = "Pet") — Java/Kotlin OpenAPI annotations
        if scan.contains(&format!("@Schema(name = \"{schema}\""))
            || scan.contains(&format!("@Schema(name=\"{schema}\""))
        {
            if !matches.contains(schema) {
                matches.push(schema.clone());
            }
            continue;
        }

        // JSON pointer in comment: #/components/schemas/Pet
        if scan.contains(&format!("#/components/schemas/{schema}"))
            || scan.contains(&format!("/components/schemas/{schema}"))
        {
            if !matches.contains(schema) {
                matches.push(schema.clone());
            }
            continue;
        }

        // TypeScript/JS: interface Pet or type Pet or export class Pet
        if (scan.contains(&format!("interface {schema}"))
            || scan.contains(&format!("type {schema} "))
            || scan.contains(&format!("class {schema}"))
            || scan.contains(&format!("struct {schema}")))
            && !matches.contains(schema)
        {
            matches.push(schema.clone());
        }
    }

    for op_id in &names.operation_ids {
        // operationId in comment or as method name
        if (scan.contains(&format!("operationId: {op_id}"))
            || scan.contains(&format!("operationId: \"{op_id}\""))
            || scan.contains(&format!("operation_id = \"{op_id}\"")))
            && !matches.contains(op_id)
        {
            matches.push(op_id.clone());
        }
    }

    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Compile error parsing ────────────────────────────────────────

    #[test]
    fn java_error() {
        let log = "src/main/java/com/example/Pet.java:42: error: cannot find symbol";
        let errors = parse_compile_errors(log);
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].file,
            PathBuf::from("src/main/java/com/example/Pet.java")
        );
        assert_eq!(errors[0].line, 42);
        assert!(errors[0].message.contains("cannot find symbol"));
    }

    #[test]
    fn java_container_path() {
        let log = "/work/.oav/generated/server/spring/src/Pet.java:10: error: missing";
        let errors = parse_compile_errors(log);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].file, PathBuf::from("src/Pet.java"));
        assert_eq!(errors[0].line, 10);
    }

    #[test]
    fn typescript_error() {
        let log = "src/models/Pet.ts(42,10): error TS2345: Argument of type 'string'";
        let errors = parse_compile_errors(log);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].file, PathBuf::from("src/models/Pet.ts"));
        assert_eq!(errors[0].line, 42);
        assert_eq!(errors[0].col, Some(10));
    }

    #[test]
    fn go_error() {
        let log = "pkg/api/pet.go:15:2: undefined: PetStore";
        let errors = parse_compile_errors(log);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].file, PathBuf::from("pkg/api/pet.go"));
        assert_eq!(errors[0].line, 15);
        assert_eq!(errors[0].col, Some(2));
        assert!(errors[0].message.contains("undefined"));
    }

    #[test]
    fn python_error() {
        let log = "  File \"models/pet.py\", line 23, in <module>";
        let errors = parse_compile_errors(log);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].file, PathBuf::from("models/pet.py"));
        assert_eq!(errors[0].line, 23);
    }

    #[test]
    fn csharp_error() {
        let log = "Models/Pet.cs(15,8): error CS0246: The type or namespace name 'Pet'";
        let errors = parse_compile_errors(log);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].file, PathBuf::from("Models/Pet.cs"));
        assert_eq!(errors[0].line, 15);
        assert_eq!(errors[0].col, Some(8));
    }

    #[test]
    fn multi_language_mixed() {
        let log = "\
src/Pet.java:5: error: semicolon expected
src/api.ts(10,1): error TS1005: ';' expected
main.go:3:1: syntax error
  File \"app.py\", line 1
";
        let errors = parse_compile_errors(log);
        assert_eq!(errors.len(), 4);
    }

    #[test]
    fn empty_and_noise() {
        let log = "\
BUILD FAILED
Some random noise
  at org.gradle.internal.something

";
        let errors = parse_compile_errors(log);
        assert!(errors.is_empty());
    }

    // ── Container path normalization ─────────────────────────────────

    #[test]
    fn normalize_strips_work_prefix() {
        assert_eq!(
            normalize_container_path("/work/src/Foo.java"),
            PathBuf::from("src/Foo.java")
        );
    }

    #[test]
    fn normalize_strips_generated_prefix() {
        assert_eq!(
            normalize_container_path("/work/.oav/generated/server/spring/src/Foo.java"),
            PathBuf::from("src/Foo.java")
        );
    }

    #[test]
    fn normalize_keeps_relative() {
        assert_eq!(
            normalize_container_path("src/Foo.java"),
            PathBuf::from("src/Foo.java")
        );
    }

    // ── SpecNames extraction ─────────────────────────────────────────

    #[test]
    fn spec_names_from_pointers() {
        let pointers = vec![
            "/components/schemas/Pet".to_string(),
            "/components/schemas/Pet/properties/name".to_string(),
            "/components/schemas/Order".to_string(),
            "/paths/~1pets/get".to_string(),
            "/paths/~1pets/get/summary".to_string(),
        ];
        let names = SpecNames::from_pointers(&pointers);
        assert_eq!(names.schemas, vec!["Pet", "Order"]);
        assert_eq!(names.api_paths, vec!["/pets"]);
    }

    #[test]
    fn path_resource_names_extracts_segments() {
        let names = SpecNames {
            api_paths: vec!["/pets".into(), "/users/{userId}/orders".into()],
            ..Default::default()
        };
        let resources = names.path_resource_names();
        assert!(resources.contains(&"pets".to_string()));
        assert!(resources.contains(&"users".to_string()));
        assert!(resources.contains(&"orders".to_string()));
        // {userId} is a parameter, not a resource
        assert!(!resources.iter().any(|r| r.contains('{')));
    }

    #[test]
    fn name_match_api_path_resource() {
        let names = SpecNames {
            api_paths: vec!["/pets".into()],
            ..Default::default()
        };
        let m = name_based_match("PetsApi", Path::new("api/PetsApi.java"), &names);
        assert!(m.contains(&"/pets".to_string()));
    }

    // ── Name-based matching ──────────────────────────────────────────

    #[test]
    fn name_match_direct() {
        let names = SpecNames {
            schemas: vec!["Pet".into()],
            ..Default::default()
        };
        let m = name_based_match("Pet", Path::new("model/Pet.java"), &names);
        assert_eq!(m, vec!["Pet"]);
    }

    #[test]
    fn name_match_suffixed_in_model_dir() {
        let names = SpecNames {
            schemas: vec!["Pet".into()],
            ..Default::default()
        };
        let m = name_based_match("PetDto", Path::new("model/PetDto.java"), &names);
        assert_eq!(m, vec!["Pet"]);
    }

    #[test]
    fn name_match_case_insensitive() {
        let names = SpecNames {
            schemas: vec!["Pet".into()],
            ..Default::default()
        };
        let m = name_based_match("pet", Path::new("models/pet.py"), &names);
        assert_eq!(m, vec!["Pet"]);
    }

    #[test]
    fn name_no_match_unrelated() {
        let names = SpecNames {
            schemas: vec!["Pet".into()],
            ..Default::default()
        };
        let m = name_based_match("Utils", Path::new("util/Utils.java"), &names);
        assert!(m.is_empty());
    }

    // ── Comment-based matching ───────────────────────────────────────

    #[test]
    fn comment_match_in_tempfile() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Pet.java");
        let mut f = std::fs::File::create(&file).unwrap();
        writeln!(f, "// Generated from #/components/schemas/Pet").unwrap();
        writeln!(f, "@Schema(name = \"Pet\")").unwrap();
        writeln!(f, "public class Pet {{}}").unwrap();

        let names = SpecNames {
            schemas: vec!["Pet".into(), "Order".into()],
            ..Default::default()
        };
        let m = comment_based_match(&file, &names);
        assert_eq!(m, vec!["Pet"]);
    }
}
