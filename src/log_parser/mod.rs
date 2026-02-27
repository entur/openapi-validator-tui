/// Lint log parsing — Spectral and Redocly stylish-format output to structured errors.
mod parse;

pub use parse::parse_lint_log;

use std::cmp::Ordering;
use std::fmt;

/// Severity level of a lint finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

impl Severity {
    fn rank(self) -> u8 {
        match self {
            Self::Error => 3,
            Self::Warning => 2,
            Self::Info => 1,
            Self::Hint => 0,
        }
    }

    fn from_str_lossy(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "error" => Self::Error,
            "warning" => Self::Warning,
            "info" | "information" => Self::Info,
            "hint" => Self::Hint,
            _ => Self::Warning,
        }
    }
}

impl Ord for Severity {
    fn cmp(&self, other: &Self) -> Ordering {
        self.rank().cmp(&other.rank())
    }
}

impl PartialOrd for Severity {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => f.write_str("error"),
            Self::Warning => f.write_str("warning"),
            Self::Info => f.write_str("info"),
            Self::Hint => f.write_str("hint"),
        }
    }
}

/// A single lint finding parsed from linter output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintError {
    pub line: usize,
    pub col: usize,
    pub severity: Severity,
    pub rule: String,
    pub message: String,
    pub json_path: Option<String>,
}
