use std::collections::HashMap;

/// A 1-based line, 0-based column location in a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    pub line: usize,
    pub col: usize,
}

/// A window of source lines around a target line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextWindow {
    pub start_line: usize,
    pub lines: Vec<String>,
    pub target_line: usize,
}

/// Index mapping JSON pointers to source locations, plus the raw source lines.
#[derive(Debug)]
pub struct SpecIndex {
    spans: HashMap<String, SourceSpan>,
    raw_lines: Vec<String>,
}

impl SpecIndex {
    pub fn new(spans: HashMap<String, SourceSpan>, raw_lines: Vec<String>) -> Self {
        Self { spans, raw_lines }
    }

    /// Look up a JSON pointer or dotted path and return its source location.
    pub fn resolve(&self, path: &str) -> Option<SourceSpan> {
        let pointer = super::parser::normalize_to_pointer(path);
        self.spans.get(&pointer).copied()
    }

    /// Extract a window of `radius` lines above and below the given 1-based line.
    pub fn context_window(&self, line: usize, radius: usize) -> Option<ContextWindow> {
        if line == 0 || line > self.raw_lines.len() {
            return None;
        }
        let start = line.saturating_sub(radius).max(1);
        let end = (line + radius).min(self.raw_lines.len());
        let lines = self.raw_lines[start - 1..end].to_vec();
        Some(ContextWindow {
            start_line: start,
            lines,
            target_line: line,
        })
    }

    pub fn line_count(&self) -> usize {
        self.raw_lines.len()
    }

    pub fn lines(&self) -> &[String] {
        &self.raw_lines
    }
}
