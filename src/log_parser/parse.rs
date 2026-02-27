use super::{LintError, Severity};

/// Parse raw stylish-format lint output (Spectral / Redocly) into structured errors.
///
/// Expects the `--format stylish` output layout:
/// ```text
/// /path/to/spec.yaml
///   2:6   warning  info-contact  Info object should contain `contact` object.
///  10:3   error    my-rule-one   Tags must have a description.                tags[0]
///
/// ✖ 2 problems (2 errors, 0 warnings, 0 infos, 0 hints)
/// ```
pub fn parse_lint_log(raw: &str) -> Vec<LintError> {
    let mut errors = Vec::new();

    for line in raw.lines() {
        // Skip blank lines.
        if line.trim().is_empty() {
            continue;
        }

        // Skip non-indented lines (file headers) and summary lines.
        if !line.starts_with(' ') && !line.starts_with('\t') {
            continue;
        }

        let trimmed = line.trim_start();

        // Skip summary lines that start with a cross mark.
        if trimmed.starts_with('✖') || trimmed.starts_with('×') {
            continue;
        }

        if let Some(err) = parse_entry(trimmed) {
            errors.push(err);
        }
    }

    errors
}

/// Parse a single trimmed entry line.
///
/// Expected format: `line:col  severity  rule-id  message  [json-path]`
fn parse_entry(trimmed: &str) -> Option<LintError> {
    let mut tokens = trimmed.split_whitespace();

    // 1. line:col
    let loc = tokens.next()?;
    let (line, col) = parse_location(loc)?;

    // 2. severity
    let severity_str = tokens.next()?;
    let severity = Severity::from_str_lossy(severity_str);

    // 3. rule-id
    let rule = tokens.next()?.to_string();

    // 4. Remaining tokens form the message, with a possible trailing json-path.
    let rest: Vec<&str> = tokens.collect();
    if rest.is_empty() {
        return Some(LintError {
            line,
            col,
            severity,
            rule,
            message: String::new(),
            json_path: None,
        });
    }

    let (message, json_path) = split_message_and_path(&rest);

    Some(LintError {
        line,
        col,
        severity,
        rule,
        message,
        json_path,
    })
}

/// Parse `"line:col"` into `(usize, usize)`.
fn parse_location(s: &str) -> Option<(usize, usize)> {
    let (l, c) = s.split_once(':')?;
    Some((l.parse().ok()?, c.parse().ok()?))
}

/// Split the remaining tokens into (message, optional json_path).
///
/// A trailing token is treated as a JSON path if it contains `/` or starts with
/// a path-like pattern (contains `[`), distinguishing it from normal message words.
fn split_message_and_path(tokens: &[&str]) -> (String, Option<String>) {
    if tokens.len() > 1 {
        let last = tokens[tokens.len() - 1];
        if looks_like_json_path(last) {
            let message = tokens[..tokens.len() - 1].join(" ");
            return (message, Some(last.to_string()));
        }
    }

    (tokens.join(" "), None)
}

/// Heuristic: does this token look like a JSON path or JSON pointer?
///
/// JSON paths: `paths./users.get`, `tags[0]`, `info.contact`
/// JSON pointers: `/paths/~1users/get`
fn looks_like_json_path(token: &str) -> bool {
    // Must not be empty.
    if token.is_empty() {
        return false;
    }
    // Starts with `/` → JSON pointer style.
    if token.starts_with('/') {
        return true;
    }
    // Contains `[` → array index notation (e.g. `tags[0]`).
    if token.contains('[') {
        return true;
    }
    // Dotted path with no spaces (e.g. `paths./users.get`, `info.contact`).
    // Only treat as path if it has at least one dot and doesn't look like a
    // normal sentence-ending word (e.g. `object.`).
    if token.contains('.') {
        let stripped = token.trim_end_matches('.');
        if stripped.contains('.') {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log_parser::Severity;

    #[test]
    fn spectral_stylish_multi_error() {
        let input = "\
/path/to/spec.yaml
  2:6   warning  info-contact       Info object should contain `contact` object.
 10:3   error    my-rule-one        Tags must have a description.                tags[0]
  5:12  error    oas3-schema        Schema should have a description.            paths./users.get

✖ 3 problems (2 errors, 1 warning, 0 infos, 0 hints)
";
        let errors = parse_lint_log(input);
        assert_eq!(errors.len(), 3);

        assert_eq!(errors[0].line, 2);
        assert_eq!(errors[0].col, 6);
        assert_eq!(errors[0].severity, Severity::Warning);
        assert_eq!(errors[0].rule, "info-contact");
        assert_eq!(
            errors[0].message,
            "Info object should contain `contact` object."
        );
        assert_eq!(errors[0].json_path, None);

        assert_eq!(errors[1].line, 10);
        assert_eq!(errors[1].col, 3);
        assert_eq!(errors[1].severity, Severity::Error);
        assert_eq!(errors[1].rule, "my-rule-one");
        assert_eq!(errors[1].message, "Tags must have a description.");
        assert_eq!(errors[1].json_path.as_deref(), Some("tags[0]"));

        assert_eq!(errors[2].line, 5);
        assert_eq!(errors[2].col, 12);
        assert_eq!(errors[2].severity, Severity::Error);
        assert_eq!(errors[2].json_path.as_deref(), Some("paths./users.get"));
    }

    #[test]
    fn redocly_stylish() {
        let input = "\
/home/user/api.yaml
  1:1   warning  no-empty-servers   Servers list should not be empty.
 42:5   error    operation-summary  Operation must have a summary.              /paths/~1pets/get

✖ 2 problems (1 error, 1 warning, 0 infos, 0 hints)
";
        let errors = parse_lint_log(input);
        assert_eq!(errors.len(), 2);

        assert_eq!(errors[0].severity, Severity::Warning);
        assert_eq!(errors[0].rule, "no-empty-servers");

        assert_eq!(errors[1].severity, Severity::Error);
        assert_eq!(errors[1].line, 42);
        assert_eq!(errors[1].col, 5);
        assert_eq!(errors[1].json_path.as_deref(), Some("/paths/~1pets/get"));
    }

    #[test]
    fn line_without_json_path() {
        let input = "  3:1  warning  some-rule  This is a message without a path.\n";
        // Wrap with a file header to make the indented line parseable in context.
        let full = format!("/spec.yaml\n{input}");
        let errors = parse_lint_log(&full);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "This is a message without a path.");
        assert_eq!(errors[0].json_path, None);
    }

    #[test]
    fn line_with_json_path() {
        let input = "/spec.yaml\n  7:14  error  path-rule  Must be valid.  paths./foo.bar\n";
        let errors = parse_lint_log(input);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "Must be valid.");
        assert_eq!(errors[0].json_path.as_deref(), Some("paths./foo.bar"));
    }

    #[test]
    fn empty_input() {
        assert!(parse_lint_log("").is_empty());
    }

    #[test]
    fn garbage_input() {
        let garbage = "this is not lint output\nrandom text\n\n";
        assert!(parse_lint_log(garbage).is_empty());
    }

    #[test]
    fn summary_line_skipped() {
        let input = "✖ 5 problems (3 errors, 2 warnings, 0 infos, 0 hints)\n";
        assert!(parse_lint_log(input).is_empty());
    }

    #[test]
    fn severity_ordering() {
        assert!(Severity::Error > Severity::Warning);
        assert!(Severity::Warning > Severity::Info);
        assert!(Severity::Info > Severity::Hint);
        assert!(Severity::Error > Severity::Hint);
    }

    #[test]
    fn severity_display() {
        assert_eq!(Severity::Error.to_string(), "error");
        assert_eq!(Severity::Warning.to_string(), "warning");
        assert_eq!(Severity::Info.to_string(), "info");
        assert_eq!(Severity::Hint.to_string(), "hint");
    }

    #[test]
    fn unknown_severity_defaults_to_warning() {
        let input = "/spec.yaml\n  1:1  banana  some-rule  A message.\n";
        let errors = parse_lint_log(input);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].severity, Severity::Warning);
    }

    #[test]
    fn info_and_hint_severities() {
        let input = "\
/spec.yaml
  1:1  info       info-rule   Info level finding.
  2:1  hint       hint-rule   Hint level finding.
  3:1  information info-rule2 Another info finding.
";
        let errors = parse_lint_log(input);
        assert_eq!(errors.len(), 3);
        assert_eq!(errors[0].severity, Severity::Info);
        assert_eq!(errors[1].severity, Severity::Hint);
        assert_eq!(errors[2].severity, Severity::Info);
    }
}
