mod convert;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use ratatui::style::Style;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

pub struct HighlightEngine {
    syntax_set: SyntaxSet,
    theme: Theme,
    cache: Option<CachedHighlight>,
}

struct CachedHighlight {
    content_hash: u64,
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
    /// Returns cached results when the content and syntax haven't changed.
    pub fn highlight_lines(
        &mut self,
        raw_lines: &[String],
        syntax_name: &str,
    ) -> &[Vec<(Style, String)>] {
        let hash = Self::hash_content(raw_lines, syntax_name);

        let needs_highlight = match &self.cache {
            Some(cached) => cached.content_hash != hash || cached.syntax_name != syntax_name,
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
                content_hash: hash,
                syntax_name: syntax_name.to_owned(),
                lines: highlighted,
            });
        }

        &self.cache.as_ref().unwrap().lines
    }

    /// Clear the cache, forcing re-highlight on the next call.
    pub fn invalidate(&mut self) {
        self.cache = None;
    }

    fn hash_content(lines: &[String], syntax_name: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        for line in lines {
            line.hash(&mut hasher);
        }
        syntax_name.hash(&mut hasher);
        hasher.finish()
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
        let result = engine.highlight_lines(&lines, "YAML");
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn cache_hit_on_repeated_call() {
        let mut engine = HighlightEngine::new();
        let lines: Vec<String> = vec!["key: value\n".into()];

        engine.highlight_lines(&lines, "YAML");
        assert!(engine.cache.is_some());

        let hash_before = engine.cache.as_ref().unwrap().content_hash;
        engine.highlight_lines(&lines, "YAML");
        let hash_after = engine.cache.as_ref().unwrap().content_hash;

        assert_eq!(hash_before, hash_after);
    }

    #[test]
    fn invalidate_forces_rehighlight() {
        let mut engine = HighlightEngine::new();
        let lines: Vec<String> = vec!["key: value\n".into()];

        engine.highlight_lines(&lines, "YAML");
        assert!(engine.cache.is_some());

        engine.invalidate();
        assert!(engine.cache.is_none());

        // Re-highlights successfully after invalidation.
        let result = engine.highlight_lines(&lines, "YAML");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn unknown_syntax_falls_back_to_plain_text() {
        let mut engine = HighlightEngine::new();
        let lines: Vec<String> = vec!["some content\n".into()];
        let result = engine.highlight_lines(&lines, "NoSuchLanguage");
        assert_eq!(result.len(), 1);
    }
}
