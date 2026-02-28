use std::collections::HashMap;

use anyhow::Result;

use super::types::{SourceSpan, SpecIndex};

/// Parse a raw spec string (YAML or prettified JSON) and build a `SpecIndex`
/// mapping JSON pointers to source line numbers.
pub fn parse_spec(raw: &str) -> Result<SpecIndex> {
    let lines: Vec<String> = raw.lines().map(String::from).collect();
    let mut spans = HashMap::new();
    // Stack of (indent_level, key_name).
    let mut stack: Vec<(usize, String)> = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Skip blank lines, comments, and flow collections.
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with('{')
            || trimmed.starts_with('[')
        {
            continue;
        }

        let indent = leading_spaces(line);

        let (key, key_col) = match extract_yaml_key(trimmed) {
            Some((k, offset)) => (k, indent + offset),
            None => continue,
        };

        // Pop entries at the same or deeper indent — we moved up or sideways.
        while let Some(&(lvl, _)) = stack.last() {
            if lvl >= indent {
                stack.pop();
            } else {
                break;
            }
        }

        stack.push((indent, key));

        let pointer = build_json_pointer(&stack);
        spans.insert(
            pointer,
            SourceSpan {
                line: idx + 1,
                col: key_col,
            },
        );
    }

    Ok(SpecIndex::new(spans, lines))
}

/// Count leading ASCII spaces (tabs count as 1 for simplicity).
fn leading_spaces(line: &str) -> usize {
    line.len() - line.trim_start_matches([' ', '\t']).len()
}

/// Extract the YAML key from a trimmed line.
///
/// Handles:
/// - `key:` / `key: value`
/// - `"key":` / `'key':` (quoted)
/// - `200:` (numeric)
/// - `- key: value` (array item — strips `- `, returns col offset = 2)
///
/// Returns `(key, col_offset)` where col_offset is the byte offset within the
/// trimmed line at which the key starts (0 for normal keys, 2 for `- key`).
fn extract_yaml_key(trimmed: &str) -> Option<(String, usize)> {
    let (effective, col_offset) = if let Some(stripped) = trimmed.strip_prefix("- ") {
        (stripped, 2)
    } else {
        (trimmed, 0)
    };

    // JSON-style: "key": or 'key':
    if effective.starts_with('"') || effective.starts_with('\'') {
        let quote = effective.as_bytes()[0] as char;
        let rest = &effective[1..];
        let end = rest.find(quote)?;
        let key = &rest[..end];
        // Must be followed by `:`
        let after = &rest[end + 1..];
        if !after.starts_with(':') {
            return None;
        }
        return Some((key.to_string(), col_offset));
    }

    // Bare key or numeric key: everything before the first `:`
    let colon = effective.find(':')?;
    // Reject lines where `:`  is inside a value (no key, e.g. `  value: with: colons`)
    // The key portion must be a valid YAML key — no spaces before the colon for bare keys.
    let candidate = &effective[..colon];
    if candidate.is_empty() {
        return None;
    }
    // Allow keys with spaces (e.g. `application/json`) but reject if the whole
    // line looks like a continuation (starts with `-` without space, etc.).
    Some((candidate.trim().to_string(), col_offset))
}

/// Build a JSON pointer string from the current stack per RFC 6901.
fn build_json_pointer(stack: &[(usize, String)]) -> String {
    if stack.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for (_, key) in stack {
        out.push('/');
        for ch in key.chars() {
            match ch {
                '~' => out.push_str("~0"),
                '/' => out.push_str("~1"),
                _ => out.push(ch),
            }
        }
    }
    out
}

/// Normalize a path (JSON pointer or dotted) to a JSON pointer.
///
/// - If it already starts with `/`, return as-is.
/// - Otherwise split on `.`, handle `[n]` bracket notation, escape segments,
///   and join as a pointer.
pub fn normalize_to_pointer(path: &str) -> String {
    if path.starts_with('/') || path.is_empty() {
        return path.to_string();
    }

    let mut pointer = String::new();
    for segment in path.split('.') {
        // Handle bracket notation: `items[0]` → segments `items`, `0`
        let mut rest = segment;
        while !rest.is_empty() {
            if let Some(bracket_start) = rest.find('[') {
                let before = &rest[..bracket_start];
                if !before.is_empty() {
                    pointer.push('/');
                    escape_pointer_segment(before, &mut pointer);
                }
                let bracket_end = rest[bracket_start..].find(']').map(|i| bracket_start + i);
                if let Some(end) = bracket_end {
                    let index = &rest[bracket_start + 1..end];
                    pointer.push('/');
                    pointer.push_str(index);
                    rest = &rest[end + 1..];
                } else {
                    // Malformed bracket — just include the rest literally.
                    pointer.push('/');
                    escape_pointer_segment(rest, &mut pointer);
                    rest = "";
                }
            } else {
                pointer.push('/');
                escape_pointer_segment(rest, &mut pointer);
                rest = "";
            }
        }
    }
    pointer
}

fn escape_pointer_segment(seg: &str, out: &mut String) {
    for ch in seg.chars() {
        match ch {
            '~' => out.push_str("~0"),
            '/' => out.push_str("~1"),
            _ => out.push(ch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- extract_yaml_key tests ----

    #[test]
    fn key_bare() {
        assert_eq!(extract_yaml_key("paths:"), Some(("paths".into(), 0)));
    }

    #[test]
    fn key_bare_with_value() {
        assert_eq!(extract_yaml_key("title: My API"), Some(("title".into(), 0)));
    }

    #[test]
    fn key_quoted_double() {
        assert_eq!(extract_yaml_key("\"/pets\":"), Some(("/pets".into(), 0)));
    }

    #[test]
    fn key_quoted_single() {
        assert_eq!(extract_yaml_key("'/pets':"), Some(("/pets".into(), 0)));
    }

    #[test]
    fn key_numeric() {
        assert_eq!(extract_yaml_key("200:"), Some(("200".into(), 0)));
    }

    #[test]
    fn key_array_item() {
        assert_eq!(extract_yaml_key("- name: Fido"), Some(("name".into(), 2)));
    }

    #[test]
    fn no_key_plain_value() {
        // A continuation line with no colon.
        assert_eq!(extract_yaml_key("just a value"), None);
    }

    #[test]
    fn no_key_comment() {
        // Comments are stripped before reaching here, but just in case:
        assert_eq!(extract_yaml_key("# comment"), None);
    }

    // ---- build_json_pointer tests ----

    #[test]
    fn pointer_empty_stack() {
        assert_eq!(build_json_pointer(&[]), "");
    }

    #[test]
    fn pointer_escapes_slash() {
        let stack = vec![(0, "paths".into()), (2, "/pets".into())];
        assert_eq!(build_json_pointer(&stack), "/paths/~1pets");
    }

    #[test]
    fn pointer_escapes_tilde() {
        let stack = vec![(0, "a~b".into())];
        assert_eq!(build_json_pointer(&stack), "/a~0b");
    }

    // ---- normalize_to_pointer tests ----

    #[test]
    fn normalize_pointer_passthrough() {
        assert_eq!(normalize_to_pointer("/paths/~1pets"), "/paths/~1pets");
    }

    #[test]
    fn normalize_dotted_path() {
        assert_eq!(normalize_to_pointer("paths./pets.get"), "/paths/~1pets/get");
    }

    #[test]
    fn normalize_bracket_notation() {
        assert_eq!(normalize_to_pointer("tags[0].name"), "/tags/0/name");
    }

    #[test]
    fn normalize_empty() {
        assert_eq!(normalize_to_pointer(""), "");
    }

    // ---- parse_spec integration tests ----

    #[test]
    fn parse_openapi_yaml() {
        let yaml = "\
openapi: 3.0.0
info:
  title: Pet Store
  version: '1.0'
paths:
  /pets:
    get:
      summary: List pets
      responses:
        '200':
          description: OK
";
        let index = parse_spec(yaml).unwrap();
        assert_eq!(
            index.resolve("/openapi"),
            Some(SourceSpan { line: 1, col: 0 })
        );
        assert_eq!(
            index.resolve("/info/title"),
            Some(SourceSpan { line: 3, col: 2 })
        );
        assert_eq!(
            index.resolve("/paths/~1pets/get"),
            Some(SourceSpan { line: 7, col: 4 })
        );
        assert_eq!(
            index.resolve("/paths/~1pets/get/summary"),
            Some(SourceSpan { line: 8, col: 6 })
        );
        assert_eq!(
            index.resolve("/paths/~1pets/get/responses/200/description"),
            Some(SourceSpan { line: 11, col: 10 })
        );
    }

    #[test]
    fn parse_nested_schemas() {
        let yaml = "\
components:
  schemas:
    Pet:
      type: object
      properties:
        name:
          type: string
        tag:
          type: string
";
        let index = parse_spec(yaml).unwrap();
        assert_eq!(
            index.resolve("/components/schemas/Pet/properties/name"),
            Some(SourceSpan { line: 6, col: 8 })
        );
        assert_eq!(
            index.resolve("/components/schemas/Pet/properties/tag"),
            Some(SourceSpan { line: 8, col: 8 })
        );
    }

    #[test]
    fn parse_json_format() {
        let json = r#"{
  "openapi": "3.0.0",
  "info": {
    "title": "Test",
    "version": "1.0"
  },
  "paths": {}
}"#;
        let index = parse_spec(json).unwrap();
        // First line is `{` — skipped (flow). "openapi" is on line 2.
        assert_eq!(
            index.resolve("/openapi"),
            Some(SourceSpan { line: 2, col: 2 })
        );
        assert_eq!(
            index.resolve("/info/title"),
            Some(SourceSpan { line: 4, col: 4 })
        );
    }

    #[test]
    fn parse_dotted_path_resolves() {
        let yaml = "\
paths:
  /pets:
    get:
      summary: List pets
";
        let index = parse_spec(yaml).unwrap();
        assert_eq!(
            index.resolve("paths./pets.get.summary"),
            Some(SourceSpan { line: 4, col: 6 })
        );
    }

    #[test]
    fn context_window_normal() {
        let yaml = "a:\nb:\nc:\nd:\ne:\nf:\ng:\n";
        let index = parse_spec(yaml).unwrap();
        let window = index.context_window(4, 2).unwrap();
        assert_eq!(window.start_line, 2);
        assert_eq!(window.target_line, 4);
        assert_eq!(window.lines, vec!["b:", "c:", "d:", "e:", "f:"]);
    }

    #[test]
    fn context_window_clamps_start() {
        let yaml = "a:\nb:\nc:\n";
        let index = parse_spec(yaml).unwrap();
        let window = index.context_window(1, 5).unwrap();
        assert_eq!(window.start_line, 1);
        assert_eq!(window.lines, vec!["a:", "b:", "c:"]);
    }

    #[test]
    fn context_window_clamps_end() {
        let yaml = "a:\nb:\nc:\n";
        let index = parse_spec(yaml).unwrap();
        let window = index.context_window(3, 5).unwrap();
        assert_eq!(window.start_line, 1);
        assert_eq!(window.lines, vec!["a:", "b:", "c:"]);
    }

    #[test]
    fn context_window_out_of_range() {
        let yaml = "a:\n";
        let index = parse_spec(yaml).unwrap();
        assert!(index.context_window(0, 2).is_none());
        assert!(index.context_window(5, 2).is_none());
    }

    #[test]
    fn unknown_pointer_returns_none() {
        let yaml = "openapi: 3.0.0\n";
        let index = parse_spec(yaml).unwrap();
        assert!(index.resolve("/nonexistent").is_none());
    }

    #[test]
    fn empty_input() {
        let index = parse_spec("").unwrap();
        assert_eq!(index.line_count(), 0);
        assert!(index.lines().is_empty());
        assert!(index.resolve("/anything").is_none());
    }
}
