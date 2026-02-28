use crate::log_parser::LintError;
use crate::spec::SpecIndex;

use super::{FixProposal, gather_context};

/// Detect the indentation used by children of `parent_line` (1-based).
///
/// Scans lines below `parent_line` for the first non-blank child and returns
/// its whitespace prefix. Falls back to parent indent + 2 spaces.
fn detect_child_indent(lines: &[String], parent_line: usize) -> String {
    let parent_idx = parent_line.saturating_sub(1);
    let parent_indent = leading_whitespace(&lines[parent_idx]);

    for line in lines.iter().skip(parent_idx + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = leading_whitespace(line);
        if indent.len() > parent_indent.len() {
            return indent;
        }
        // Reached a sibling or parent — no children found.
        break;
    }

    // Fallback: parent indent + 2 spaces.
    format!("{parent_indent}  ")
}

fn leading_whitespace(line: &str) -> String {
    line.chars()
        .take_while(|c| c.is_ascii_whitespace())
        .collect()
}

/// Resolve the operation key line (e.g. `get:`) from an error's json_path.
///
/// For paths like `/paths/~1pets/get`, resolves to the line of `get:` and
/// extracts the operationId from the child `operationId:` field if present.
fn resolve_operation_context(
    error: &LintError,
    spec_index: &SpecIndex,
    lines: &[String],
) -> Option<(usize, String)> {
    let json_path = error.json_path.as_deref()?;

    // The json_path should point to the operation (e.g. /paths/~1pets/get).
    let span = spec_index.resolve(json_path)?;
    let op_line = span.line;

    if op_line == 0 || op_line > lines.len() {
        return None;
    }

    // Try to extract operationId from child fields.
    let child_indent = detect_child_indent(lines, op_line);
    let op_id = find_child_field_value(lines, op_line, &child_indent, "operationId")
        .unwrap_or_else(|| {
            // Fall back to the HTTP method as identifier.
            lines[op_line - 1].trim().trim_end_matches(':').to_string()
        });

    Some((op_line, op_id))
}

/// Find the value of a child field below `parent_line` at the given indent.
fn find_child_field_value(
    lines: &[String],
    parent_line: usize,
    child_indent: &str,
    field_name: &str,
) -> Option<String> {
    for line in lines.iter().skip(parent_line) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = leading_whitespace(line);
        if indent.len() < child_indent.len() {
            break; // Left the operation block.
        }
        if indent.len() == child_indent.len()
            && let Some(rest) = trimmed.strip_prefix(&format!("{field_name}:"))
        {
            return Some(rest.trim().to_string());
        }
    }
    None
}

/// Find the last child line of a block starting at `parent_line` (1-based).
///
/// Returns the 1-based line number of the last child (or the parent itself if
/// no children are found).
fn last_child_line(lines: &[String], parent_line: usize) -> usize {
    let parent_idx = parent_line.saturating_sub(1);
    let parent_indent_len = leading_whitespace(&lines[parent_idx]).len();
    let mut last = parent_line;

    for (i, line) in lines.iter().enumerate().skip(parent_idx + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if leading_whitespace(line).len() <= parent_indent_len {
            break;
        }
        last = i + 1; // 1-based
    }

    last
}

// ── Rule generators ──────────────────────────────────────────────────────

pub fn propose_operation_summary(
    error: &LintError,
    spec_index: &SpecIndex,
    lines: &[String],
) -> Option<FixProposal> {
    let (op_line, op_id) = resolve_operation_context(error, spec_index, lines)?;
    let indent = detect_child_indent(lines, op_line);
    let inserted = vec![format!("{indent}summary: \"{op_id} summary\"")];
    let (ctx_before, ctx_after) = gather_context(lines, op_line + 1, 3);

    Some(FixProposal {
        rule: error.rule.clone(),
        description: "Add 'summary' field to the operation".into(),
        target_line: op_line,
        context_before: ctx_before,
        inserted,
        context_after: ctx_after,
    })
}

pub fn propose_operation_description(
    error: &LintError,
    spec_index: &SpecIndex,
    lines: &[String],
) -> Option<FixProposal> {
    let (op_line, op_id) = resolve_operation_context(error, spec_index, lines)?;
    let indent = detect_child_indent(lines, op_line);
    let inserted = vec![format!("{indent}description: \"{op_id} description\"")];
    let (ctx_before, ctx_after) = gather_context(lines, op_line + 1, 3);

    Some(FixProposal {
        rule: error.rule.clone(),
        description: "Add 'description' field to the operation".into(),
        target_line: op_line,
        context_before: ctx_before,
        inserted,
        context_after: ctx_after,
    })
}

pub fn propose_info_contact(
    error: &LintError,
    spec_index: &SpecIndex,
    lines: &[String],
) -> Option<FixProposal> {
    let span = spec_index.resolve("/info")?;
    let info_line = span.line;
    let child_indent = detect_child_indent(lines, info_line);
    let nested_indent = format!("{child_indent}  ");
    let target = last_child_line(lines, info_line);

    let inserted = vec![
        format!("{child_indent}contact:"),
        format!("{nested_indent}name: \"\""),
        format!("{nested_indent}url: \"\""),
    ];
    let (ctx_before, ctx_after) = gather_context(lines, target + 1, 3);

    Some(FixProposal {
        rule: error.rule.clone(),
        description: "Add 'contact' block under /info".into(),
        target_line: target,
        context_before: ctx_before,
        inserted,
        context_after: ctx_after,
    })
}

pub fn propose_info_license(
    error: &LintError,
    spec_index: &SpecIndex,
    lines: &[String],
) -> Option<FixProposal> {
    let span = spec_index.resolve("/info")?;
    let info_line = span.line;
    let child_indent = detect_child_indent(lines, info_line);
    let nested_indent = format!("{child_indent}  ");
    let target = last_child_line(lines, info_line);

    let inserted = vec![
        format!("{child_indent}license:"),
        format!("{nested_indent}name: \"\""),
    ];
    let (ctx_before, ctx_after) = gather_context(lines, target + 1, 3);

    Some(FixProposal {
        rule: error.rule.clone(),
        description: "Add 'license' block under /info".into(),
        target_line: target,
        context_before: ctx_before,
        inserted,
        context_after: ctx_after,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log_parser::{LintError, Severity};
    use crate::spec::parse_spec;

    fn make_error(rule: &str, json_path: Option<&str>) -> LintError {
        LintError {
            line: 1,
            col: 0,
            severity: Severity::Error,
            rule: rule.into(),
            message: format!("{rule} message"),
            json_path: json_path.map(String::from),
        }
    }

    const PETSTORE_YAML: &str = "\
openapi: 3.0.0
info:
  title: Pet Store
  version: '1.0'
paths:
  /pets:
    get:
      operationId: listPets
      tags:
        - pets
      responses:
        '200':
          description: OK
";

    #[test]
    fn detect_child_indent_normal() {
        let lines: Vec<String> = PETSTORE_YAML.lines().map(String::from).collect();
        // info: is line 2 (1-based), children are indented with 2 spaces.
        assert_eq!(detect_child_indent(&lines, 2), "  ");
    }

    #[test]
    fn detect_child_indent_deeper() {
        let lines: Vec<String> = PETSTORE_YAML.lines().map(String::from).collect();
        // get: is line 7 (1-based), children are indented with 6 spaces.
        assert_eq!(detect_child_indent(&lines, 7), "      ");
    }

    #[test]
    fn detect_child_indent_fallback() {
        let lines = vec!["leaf_key: value".to_string()];
        // No children, so fallback = parent indent (0) + 2.
        assert_eq!(detect_child_indent(&lines, 1), "  ");
    }

    #[test]
    fn last_child_line_info_block() {
        let lines: Vec<String> = PETSTORE_YAML.lines().map(String::from).collect();
        // info: is line 2, its children are title (3) and version (4).
        assert_eq!(last_child_line(&lines, 2), 4);
    }

    #[test]
    fn last_child_line_leaf() {
        let lines: Vec<String> = PETSTORE_YAML.lines().map(String::from).collect();
        // openapi: is line 1, no children.
        assert_eq!(last_child_line(&lines, 1), 1);
    }

    #[test]
    fn propose_operation_summary_generates_fix() {
        let lines: Vec<String> = PETSTORE_YAML.lines().map(String::from).collect();
        let index = parse_spec(PETSTORE_YAML).unwrap();
        let error = make_error("operation-summary", Some("/paths/~1pets/get"));

        let proposal = propose_operation_summary(&error, &index, &lines).unwrap();
        assert_eq!(proposal.rule, "operation-summary");
        assert_eq!(proposal.target_line, 7); // after `get:`
        assert_eq!(proposal.inserted.len(), 1);
        assert!(proposal.inserted[0].contains("summary:"));
        assert!(proposal.inserted[0].contains("listPets"));
    }

    #[test]
    fn propose_operation_description_generates_fix() {
        let lines: Vec<String> = PETSTORE_YAML.lines().map(String::from).collect();
        let index = parse_spec(PETSTORE_YAML).unwrap();
        let error = make_error("operation-description", Some("/paths/~1pets/get"));

        let proposal = propose_operation_description(&error, &index, &lines).unwrap();
        assert_eq!(proposal.target_line, 7);
        assert!(proposal.inserted[0].contains("description:"));
        assert!(proposal.inserted[0].contains("listPets"));
    }

    #[test]
    fn propose_info_contact_generates_fix() {
        let lines: Vec<String> = PETSTORE_YAML.lines().map(String::from).collect();
        let index = parse_spec(PETSTORE_YAML).unwrap();
        let error = make_error("info-contact", None);

        let proposal = propose_info_contact(&error, &index, &lines).unwrap();
        assert_eq!(proposal.target_line, 4); // after last child of info
        assert_eq!(proposal.inserted.len(), 3);
        assert!(proposal.inserted[0].contains("contact:"));
        assert!(proposal.inserted[1].contains("name:"));
        assert!(proposal.inserted[2].contains("url:"));
    }

    #[test]
    fn propose_info_license_generates_fix() {
        let lines: Vec<String> = PETSTORE_YAML.lines().map(String::from).collect();
        let index = parse_spec(PETSTORE_YAML).unwrap();
        let error = make_error("info-license", None);

        let proposal = propose_info_license(&error, &index, &lines).unwrap();
        assert_eq!(proposal.target_line, 4);
        assert_eq!(proposal.inserted.len(), 2);
        assert!(proposal.inserted[0].contains("license:"));
        assert!(proposal.inserted[1].contains("name:"));
    }

    #[test]
    fn propose_operation_summary_no_json_path_returns_none() {
        let lines: Vec<String> = PETSTORE_YAML.lines().map(String::from).collect();
        let index = parse_spec(PETSTORE_YAML).unwrap();
        let error = make_error("operation-summary", None);

        assert!(propose_operation_summary(&error, &index, &lines).is_none());
    }

    #[test]
    fn propose_operation_summary_bad_path_returns_none() {
        let lines: Vec<String> = PETSTORE_YAML.lines().map(String::from).collect();
        let index = parse_spec(PETSTORE_YAML).unwrap();
        let error = make_error("operation-summary", Some("/nonexistent/path"));

        assert!(propose_operation_summary(&error, &index, &lines).is_none());
    }

    #[test]
    fn propose_info_contact_no_info_block_returns_none() {
        let yaml = "openapi: 3.0.0\npaths: {}\n";
        let lines: Vec<String> = yaml.lines().map(String::from).collect();
        let index = parse_spec(yaml).unwrap();
        let error = make_error("info-contact", None);

        assert!(propose_info_contact(&error, &index, &lines).is_none());
    }

    #[test]
    fn operation_summary_without_operation_id_uses_method() {
        let yaml = "\
openapi: 3.0.0
info:
  title: Test
  version: '1.0'
paths:
  /pets:
    get:
      tags:
        - pets
";
        let lines: Vec<String> = yaml.lines().map(String::from).collect();
        let index = parse_spec(yaml).unwrap();
        let error = make_error("operation-summary", Some("/paths/~1pets/get"));

        let proposal = propose_operation_summary(&error, &index, &lines).unwrap();
        // Without operationId, should fall back to HTTP method.
        assert!(proposal.inserted[0].contains("get summary"));
    }
}
