use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::App;
use crate::app::trace::CompileError;
use crate::spec::SpecIndex;
use crate::ui::style::{COLOR_GUTTER, STYLE_HIGHLIGHT_LINE, make_block};

pub fn draw_spec_context(frame: &mut Frame, app: &App, area: Rect, focused: bool) {
    let block = make_block("Spec Context", focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 1 || inner.width < 6 {
        return;
    }

    let spec_index = match &app.spec_index {
        Some(idx) => idx,
        None => {
            render_empty(frame, inner);
            return;
        }
    };

    // Resolve target line: lint errors first, then compile errors.
    let target_line =
        resolve_lint_target(app, spec_index).or_else(|| resolve_compile_target(app, spec_index));

    let radius = (inner.height as usize) / 2;

    let Some(target) = target_line else {
        render_empty(frame, inner);
        return;
    };

    let Some(window) = spec_index.context_window(target, radius) else {
        render_empty(frame, inner);
        return;
    };

    // Determine syntax from file extension.
    let syntax_name = app
        .spec_path
        .as_ref()
        .and_then(|p| p.extension())
        .and_then(|ext| ext.to_str())
        .map(|ext| match ext.to_ascii_lowercase().as_str() {
            "json" => "JSON",
            _ => "YAML",
        })
        .unwrap_or("YAML");

    // Hold the engine borrow through span construction and render so we can
    // reference cached Strings directly (via Cow::Borrowed) instead of cloning.
    let mut engine = app.highlight_engine.borrow_mut();
    let all_highlighted =
        engine.highlight_lines(spec_index.lines(), syntax_name, spec_index.version());
    let start_idx = window.start_line - 1;

    let lines: Vec<Line> = window
        .lines
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let line_num = window.start_line + i;
            let gutter = Span::styled(format!("{line_num:>4} "), Style::default().fg(COLOR_GUTTER));

            let is_target = line_num == window.target_line;

            let mut spans = vec![gutter];

            if let Some(segments) = all_highlighted.get(start_idx + i) {
                for (style, text) in segments {
                    let style = if is_target {
                        style.patch(STYLE_HIGHLIGHT_LINE)
                    } else {
                        *style
                    };
                    spans.push(Span::styled(text.as_str(), style));
                }
            }

            Line::from(spans)
        })
        .collect();

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((app.spec_scroll, 0));

    frame.render_widget(paragraph, inner);
}

fn render_empty(frame: &mut Frame, area: Rect) {
    let empty = Paragraph::new(Line::from(Span::styled(
        "No spec context available",
        Style::default().fg(Color::DarkGray),
    )));
    frame.render_widget(empty, area);
}

/// Resolve spec target line from a selected lint error.
fn resolve_lint_target(app: &App, spec_index: &SpecIndex) -> Option<usize> {
    app.selected_error().and_then(|err| {
        err.json_path
            .as_ref()
            .and_then(|path| spec_index.resolve(path))
            .map(|span| span.line)
            .or(Some(err.line).filter(|&l| l > 0))
    })
}

/// Resolve spec target line from a selected compile error.
///
/// Uses the compile error's file path to infer which spec construct it relates
/// to, then resolves that construct's location in the spec index.
fn resolve_compile_target(app: &App, spec_index: &SpecIndex) -> Option<usize> {
    let err = app.selected_compile_error()?;
    resolve_compile_error_to_spec(err, spec_index)
}

/// Map a compile error back to its originating spec line.
///
/// Strategy: extract the file stem (e.g. "Pet" from `src/models/Pet.java`) and
/// check if it matches a schema name or API resource in the spec's JSON pointers.
fn resolve_compile_error_to_spec(err: &CompileError, spec_index: &SpecIndex) -> Option<usize> {
    let stem = err.file.file_stem()?.to_str()?;
    let stem_lower = stem.to_ascii_lowercase();

    // Collect pointers once for matching.
    let pointers = spec_index.pointers();

    // Try exact schema match: "Pet" → /components/schemas/Pet
    for ptr in &pointers {
        let parts: Vec<&str> = ptr.split('/').collect();
        if parts.len() == 4
            && parts[1] == "components"
            && parts[2] == "schemas"
            && (parts[3].to_ascii_lowercase() == stem_lower
                || stem_lower.starts_with(&parts[3].to_ascii_lowercase())
                    && is_model_suffix(&stem_lower[parts[3].len()..]))
        {
            return spec_index.resolve(ptr).map(|s| s.line);
        }
    }

    // Try API path match: "PetsApi" → /paths/~1pets
    let stem_no_suffix = stem_lower
        .strip_suffix("api")
        .or_else(|| stem_lower.strip_suffix("controller"))
        .or_else(|| stem_lower.strip_suffix("service"))
        .or_else(|| stem_lower.strip_suffix("handler"))
        .unwrap_or(&stem_lower);

    for ptr in &pointers {
        let parts: Vec<&str> = ptr.split('/').collect();
        if parts.len() >= 3 && parts[1] == "paths" {
            let path_decoded = parts[2].replace("~1", "/").replace("~0", "~");
            for segment in path_decoded.split('/') {
                if !segment.is_empty()
                    && !segment.starts_with('{')
                    && segment.to_ascii_lowercase() == *stem_no_suffix
                {
                    return spec_index.resolve(ptr).map(|s| s.line);
                }
            }
        }
    }

    None
}

/// Check if the remainder after a schema name is a common model suffix.
fn is_model_suffix(remainder: &str) -> bool {
    matches!(
        remainder,
        "dto" | "model" | "entity" | "response" | "request" | "schema" | "vo" | "bean"
    )
}
