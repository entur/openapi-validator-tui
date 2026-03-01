use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Wrap};

use crate::app::App;
use crate::app::diff::{ChangeKind, DiffLine, DiffPanel};
use crate::ui::style::{COLOR_GUTTER, COLOR_SELECTED_BG, make_block};

pub fn draw_diff_browser(frame: &mut Frame, app: &App, area: Rect) {
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    draw_change_list(frame, app, horizontal[0]);
    draw_diff_content(frame, app, horizontal[1]);
}

fn draw_change_list(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.browser.diff_state.focus == DiffPanel::FileList;
    let diff = app.browser.diff_state.active_diff();
    let file_count = diff.map(|d| d.files.len()).unwrap_or(0);

    let title = format!("Changes ({file_count} files)");
    let block = make_block(&title, focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 || inner.width < 4 {
        return;
    }

    let Some(diff) = diff else {
        let empty = Paragraph::new(Line::from(Span::styled(
            "No diff data available",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty, inner);
        return;
    };

    if diff.files.is_empty() {
        let empty = Paragraph::new(Line::from(Span::styled(
            "No changes detected",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty, inner);
        return;
    }

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    let gen_label = format!("{}/{}", diff.generator, diff.scope);
    let gen_line = Paragraph::new(Line::from(Span::styled(
        gen_label,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    frame.render_widget(gen_line, sections[0]);

    let items: Vec<ListItem> = diff
        .files
        .iter()
        .map(|f| {
            let (marker, color) = match f.kind {
                ChangeKind::Added => ("[A]", Color::Green),
                ChangeKind::Modified => ("[M]", Color::Yellow),
                ChangeKind::Deleted => ("[D]", Color::Red),
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{marker} "),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(&*f.rel_path, Style::default().fg(Color::White)),
            ]))
        })
        .collect();

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(COLOR_SELECTED_BG)
            .add_modifier(Modifier::BOLD),
    );

    let mut list_state = ListState::default();
    list_state.select(Some(app.browser.diff_state.file_index));

    frame.render_stateful_widget(list, sections[1], &mut list_state);
}

fn draw_diff_content(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.browser.diff_state.focus == DiffPanel::DiffContent;
    let diff = app.browser.diff_state.active_diff();
    let file = diff.and_then(|d| d.files.get(app.browser.diff_state.file_index));

    let title = file.map(|f| f.rel_path.as_str()).unwrap_or("Diff");
    let block = make_block(title, focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 1 || inner.width < 6 {
        return;
    }

    let Some(file) = file else {
        let empty = Paragraph::new(Line::from(Span::styled(
            "Select a file to view diff",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty, inner);
        return;
    };

    let display_lines: Vec<Line> = file
        .lines
        .iter()
        .enumerate()
        .map(|(i, diff_line)| {
            let line_num = i + 1;
            let gutter = Span::styled(format!("{line_num:>4} "), Style::default().fg(COLOR_GUTTER));
            match diff_line {
                DiffLine::HunkHeader(text) => Line::from(vec![
                    gutter,
                    Span::styled(
                        text.to_string(),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                DiffLine::Insert(text) => Line::from(vec![
                    gutter,
                    Span::styled(format!("+ {text}"), Style::default().fg(Color::Green)),
                ]),
                DiffLine::Delete(text) => Line::from(vec![
                    gutter,
                    Span::styled(format!("- {text}"), Style::default().fg(Color::Red)),
                ]),
                DiffLine::Context(text) => Line::from(vec![
                    gutter,
                    Span::styled(format!("  {text}"), Style::default().fg(Color::DarkGray)),
                ]),
            }
        })
        .collect();

    let paragraph = Paragraph::new(display_lines)
        .wrap(Wrap { trim: false })
        .scroll((app.browser.diff_state.scroll, 0));

    frame.render_widget(paragraph, inner);
}
