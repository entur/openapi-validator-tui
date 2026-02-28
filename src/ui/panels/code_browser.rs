use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Tabs, Wrap};

use crate::app::browser::syntax_name_for_path;
use crate::app::{App, BrowserPanel};
use crate::ui::style::{COLOR_GUTTER, COLOR_SELECTED_BG, make_block};

pub fn draw_code_browser(frame: &mut Frame, app: &App, area: Rect) {
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    draw_file_tree(frame, app, horizontal[0]);
    draw_file_content(frame, app, horizontal[1]);
}

fn draw_file_tree(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.browser.browser_focus == BrowserPanel::FileTree;
    let block = make_block("Files", focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 || inner.width < 4 {
        return;
    }

    if app.browser.generators.is_empty() {
        let empty = Paragraph::new(Line::from(Span::styled(
            "No generated output available",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty, inner);
        return;
    }

    // Split inner into tab bar (1 line) + file list (rest).
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    // Generator tab bar.
    let tab_titles: Vec<Line> = app
        .browser
        .generators
        .iter()
        .map(|(generator, scope)| Line::from(format!("{generator}/{scope}")))
        .collect();

    let tabs = Tabs::new(tab_titles)
        .select(app.browser.generator_index)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider("│");

    frame.render_widget(tabs, sections[0]);

    // File tree list.
    if app.browser.file_tree.is_empty() {
        let empty = Paragraph::new(Line::from(Span::styled(
            "Empty generator output",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty, sections[1]);
        return;
    }

    let items: Vec<ListItem> = app
        .browser
        .file_tree
        .iter()
        .map(|entry| {
            let indent = "  ".repeat(entry.depth);
            let icon = if entry.is_dir { "▸ " } else { "  " };
            let style = if entry.is_dir {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(Span::styled(
                format!("{indent}{icon}{}", entry.name),
                style,
            )))
        })
        .collect();

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(COLOR_SELECTED_BG)
            .add_modifier(Modifier::BOLD),
    );

    let mut list_state = ListState::default();
    list_state.select(Some(app.browser.file_index));

    frame.render_stateful_widget(list, sections[1], &mut list_state);
}

fn draw_file_content(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.browser.browser_focus == BrowserPanel::FileContent;

    let title = match app.browser.file_tree.get(app.browser.file_index) {
        Some(entry) if app.browser.file_content.is_some() && !entry.is_dir => entry.name.as_str(),
        _ => "Content",
    };

    let block = make_block(title, focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 1 || inner.width < 6 {
        return;
    }

    let lines = match &app.browser.file_content {
        Some(content) => content,
        None => {
            let empty = Paragraph::new(Line::from(Span::styled(
                "Press Enter to open a file",
                Style::default().fg(Color::DarkGray),
            )));
            frame.render_widget(empty, inner);
            return;
        }
    };

    // Determine syntax from the currently selected file's path.
    let syntax_name = app
        .browser
        .file_tree
        .get(app.browser.file_index)
        .map(|e| syntax_name_for_path(&e.path))
        .unwrap_or("Plain Text");

    let mut engine = app.browser.highlight_engine.borrow_mut();
    let highlighted = engine.highlight_lines(lines, syntax_name, app.browser.content_version);

    let display_lines: Vec<Line> = highlighted
        .iter()
        .enumerate()
        .map(|(i, segments)| {
            let line_num = i + 1;
            let gutter = Span::styled(format!("{line_num:>4} "), Style::default().fg(COLOR_GUTTER));
            let mut spans = vec![gutter];
            for (style, text) in segments {
                spans.push(Span::styled(text.as_str(), *style));
            }
            Line::from(spans)
        })
        .collect();

    let paragraph = Paragraph::new(display_lines)
        .wrap(Wrap { trim: false })
        .scroll((app.browser.file_scroll, 0));

    frame.render_widget(paragraph, inner);
}
