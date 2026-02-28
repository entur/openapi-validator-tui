// Fix workflow — propose and apply mechanical fixes for lint errors.
mod rules;

use std::path::Path;

use anyhow::Result;

use crate::log_parser::LintError;
use crate::spec::SpecIndex;

/// A proposed fix for a lint error, ready for preview and application.
pub struct FixProposal {
    /// The lint rule that triggered this fix.
    pub rule: String,
    /// Human-readable description of what the fix does.
    pub description: String,
    /// 1-based line number; new lines are inserted after this line.
    pub target_line: usize,
    /// A few lines before the insertion point (for diff preview).
    pub context_before: Vec<String>,
    /// The new lines to insert.
    pub inserted: Vec<String>,
    /// A few lines after the insertion point (for diff preview).
    pub context_after: Vec<String>,
}

/// Try to generate a fix proposal for the given lint error.
///
/// Dispatches on `error.rule` to rule-specific generators. Returns `None` if
/// the rule is not supported or the error lacks sufficient context.
pub fn propose_fix(
    error: &LintError,
    spec_index: &SpecIndex,
    spec_path: &Path,
) -> Option<FixProposal> {
    let lines = read_spec_lines(spec_path)?;

    match error.rule.as_str() {
        "operation-summary" => rules::propose_operation_summary(error, spec_index, &lines),
        "operation-description" => rules::propose_operation_description(error, spec_index, &lines),
        "info-contact" => rules::propose_info_contact(error, spec_index, &lines),
        "info-license" => rules::propose_info_license(error, spec_index, &lines),
        _ => None,
    }
}

/// Apply a fix proposal by inserting lines into the spec file.
pub fn apply_fix(proposal: &FixProposal, spec_path: &Path) -> Result<()> {
    let content = std::fs::read_to_string(spec_path)?;
    let mut lines: Vec<String> = content.lines().map(String::from).collect();

    // Handle trailing newline: if the original file ended with a newline and
    // our split dropped it, we'll restore it when writing back.
    let trailing_newline = content.ends_with('\n');

    if proposal.target_line > lines.len() {
        anyhow::bail!(
            "target_line {} is beyond file length {}",
            proposal.target_line,
            lines.len()
        );
    }

    // Insert after target_line (1-based), so the vec index is target_line.
    for (i, new_line) in proposal.inserted.iter().enumerate() {
        lines.insert(proposal.target_line + i, new_line.clone());
    }

    let mut output = lines.join("\n");
    if trailing_newline {
        output.push('\n');
    }
    std::fs::write(spec_path, output)?;
    Ok(())
}

fn read_spec_lines(spec_path: &Path) -> Option<Vec<String>> {
    let content = std::fs::read_to_string(spec_path).ok()?;
    Some(content.lines().map(String::from).collect())
}

/// Gather context lines around `target_line` (1-based) for a diff preview.
fn gather_context(
    lines: &[String],
    target_line: usize,
    radius: usize,
) -> (Vec<String>, Vec<String>) {
    let idx = target_line.saturating_sub(1); // 0-based
    let start = idx.saturating_sub(radius);
    let before: Vec<String> = lines[start..idx.min(lines.len())].to_vec();
    let after_start = idx.min(lines.len());
    let after_end = (idx + radius).min(lines.len());
    let after: Vec<String> = lines[after_start..after_end].to_vec();
    (before, after)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_proposal(target_line: usize, inserted: Vec<&str>) -> FixProposal {
        FixProposal {
            rule: "test-rule".into(),
            description: "test fix".into(),
            target_line,
            context_before: vec![],
            inserted: inserted.into_iter().map(String::from).collect(),
            context_after: vec![],
        }
    }

    #[test]
    fn apply_fix_inserts_after_target_line() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "line1").unwrap();
        writeln!(f, "line2").unwrap();
        writeln!(f, "line3").unwrap();

        let proposal = make_proposal(2, vec!["  inserted_a", "  inserted_b"]);
        apply_fix(&proposal, f.path()).unwrap();

        let result = std::fs::read_to_string(f.path()).unwrap();
        let result_lines: Vec<&str> = result.lines().collect();
        assert_eq!(
            result_lines,
            vec!["line1", "line2", "  inserted_a", "  inserted_b", "line3"]
        );
    }

    #[test]
    fn apply_fix_at_end_of_file() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "line1").unwrap();
        writeln!(f, "line2").unwrap();

        let proposal = make_proposal(2, vec!["  appended"]);
        apply_fix(&proposal, f.path()).unwrap();

        let result = std::fs::read_to_string(f.path()).unwrap();
        let result_lines: Vec<&str> = result.lines().collect();
        assert_eq!(result_lines, vec!["line1", "line2", "  appended"]);
    }

    #[test]
    fn apply_fix_preserves_trailing_newline() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "line1\nline2\n").unwrap();

        let proposal = make_proposal(1, vec!["  new"]);
        apply_fix(&proposal, f.path()).unwrap();

        let result = std::fs::read_to_string(f.path()).unwrap();
        assert!(result.ends_with('\n'));
    }

    #[test]
    fn apply_fix_target_beyond_file_errors() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "line1").unwrap();

        let proposal = make_proposal(99, vec!["  bad"]);
        assert!(apply_fix(&proposal, f.path()).is_err());
    }

    #[test]
    fn gather_context_normal() {
        let lines: Vec<String> = (1..=10).map(|i| format!("line{i}")).collect();
        let (before, after) = gather_context(&lines, 5, 2);
        assert_eq!(before, vec!["line3", "line4"]);
        assert_eq!(after, vec!["line5", "line6"]);
    }

    #[test]
    fn gather_context_clamps_at_start() {
        let lines: Vec<String> = (1..=5).map(|i| format!("line{i}")).collect();
        let (before, after) = gather_context(&lines, 1, 3);
        assert_eq!(before, Vec::<String>::new());
        assert_eq!(after, vec!["line1", "line2", "line3"]);
    }

    #[test]
    fn gather_context_clamps_at_end() {
        let lines: Vec<String> = (1..=5).map(|i| format!("line{i}")).collect();
        let (before, after) = gather_context(&lines, 5, 3);
        assert_eq!(before, vec!["line2", "line3", "line4"]);
        assert_eq!(after, vec!["line5"]);
    }

    #[test]
    fn propose_fix_returns_none_for_unknown_rule() {
        let error = crate::log_parser::LintError {
            line: 1,
            col: 0,
            severity: crate::log_parser::Severity::Error,
            rule: "unknown-rule".into(),
            message: "some message".into(),
            json_path: None,
        };
        let raw = "openapi: 3.0.0\n";
        let index = crate::spec::parse_spec(raw).unwrap();

        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{raw}").unwrap();

        assert!(propose_fix(&error, &index, f.path()).is_none());
    }
}
