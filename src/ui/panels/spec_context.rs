use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::App;
use crate::ui::style::{COLOR_GUTTER, COLOR_SELECTED_BG, make_block};

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
            let empty = Paragraph::new(Line::from(Span::styled(
                "No spec context available",
                Style::default().fg(Color::DarkGray),
            )));
            frame.render_widget(empty, inner);
            return;
        }
    };

    // Resolve the target line from the selected error.
    let target_line = app.selected_error().and_then(|err| {
        // Try json_path resolution first, fall back to the error's line number.
        if let Some(ref path) = err.json_path {
            spec_index.resolve(path).map(|span| span.line)
        } else if err.line > 0 {
            Some(err.line)
        } else {
            None
        }
    });

    let radius = (inner.height as usize) / 2;

    let Some(target) = target_line else {
        let empty = Paragraph::new(Line::from(Span::styled(
            "No spec context available",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty, inner);
        return;
    };

    let Some(window) = spec_index.context_window(target, radius) else {
        let empty = Paragraph::new(Line::from(Span::styled(
            "No spec context available",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty, inner);
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
    let all_highlighted = engine.highlight_lines(spec_index.lines(), syntax_name, spec_index.version());
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
                        style.bg(COLOR_SELECTED_BG).add_modifier(Modifier::BOLD)
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
