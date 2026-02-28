mod convert;

use ratatui::style::Style;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

pub struct HighlightEngine {
    syntax_set: SyntaxSet,
    theme: Theme,
    cache: Option<CachedHighlight>,
}

struct CachedHighlight {
    version: u64,
    syntax_name: String,
    lines: Vec<Vec<(Style, String)>>,
}

impl HighlightEngine {
    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme = ThemeSet::load_defaults().themes["base16-ocean.dark"].clone();
        Self {
            syntax_set,
            theme,
            cache: None,
        }
    }

    /// Highlight raw lines using the given syntax name.
    ///
    /// Cache is keyed on `version` (from `SpecIndex::version()`) and `syntax_name`,
    /// making cache-hit checks O(1) regardless of file size.
    pub fn highlight_lines(
        &mut self,
        raw_lines: &[String],
        syntax_name: &str,
        version: u64,
    ) -> &[Vec<(Style, String)>] {
        let needs_highlight = match &self.cache {
            Some(cached) => cached.version != version || cached.syntax_name != syntax_name,
            None => true,
        };

        if needs_highlight {
            let syntax = self
                .syntax_set
                .find_syntax_by_name(syntax_name)
                .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

            let mut highlighter = syntect::easy::HighlightLines::new(syntax, &self.theme);

            let highlighted: Vec<Vec<(Style, String)>> = raw_lines
                .iter()
                .map(|line| {
                    let ranges = highlighter
                        .highlight_line(line, &self.syntax_set)
                        .unwrap_or_default();
                    convert::syntect_to_ratatui_spans(&ranges)
                })
                .collect();

            self.cache = Some(CachedHighlight {
                version,
                syntax_name: syntax_name.to_owned(),
                lines: highlighted,
            });
        }

        &self.cache.as_ref().unwrap().lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_snippet_returns_correct_line_count() {
        let mut engine = HighlightEngine::new();
        let lines: Vec<String> = vec![
            "openapi: '3.0.0'\n".into(),
            "info:\n".into(),
            "  title: Test\n".into(),
        ];
        let result = engine.highlight_lines(&lines, "YAML", 0);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn cache_hit_on_same_version() {
        let mut engine = HighlightEngine::new();
        let lines: Vec<String> = vec!["key: value\n".into()];

        engine.highlight_lines(&lines, "YAML", 42);
        assert!(engine.cache.is_some());

        // Same version → cache hit (even with different Vec instance).
        let lines2: Vec<String> = vec!["different: content\n".into()];
        let result = engine.highlight_lines(&lines2, "YAML", 42);
        // Returns the original cached result, not re-highlighted.
        assert_eq!(result.len(), 1);
        assert_eq!(engine.cache.as_ref().unwrap().version, 42);
    }

    #[test]
    fn new_version_forces_rehighlight() {
        let mut engine = HighlightEngine::new();
        let lines: Vec<String> = vec!["key: value\n".into()];

        engine.highlight_lines(&lines, "YAML", 1);
        assert!(engine.cache.is_some());

        let lines2: Vec<String> = vec!["a: b\n".into(), "c: d\n".into()];
        let result = engine.highlight_lines(&lines2, "YAML", 2);
        assert_eq!(result.len(), 2);
        assert_eq!(engine.cache.as_ref().unwrap().version, 2);
    }

    #[test]
    fn unknown_syntax_falls_back_to_plain_text() {
        let mut engine = HighlightEngine::new();
        let lines: Vec<String> = vec!["some content\n".into()];
        let result = engine.highlight_lines(&lines, "NoSuchLanguage", 0);
        assert_eq!(result.len(), 1);
    }
}
