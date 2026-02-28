use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Tabs, Wrap};

use crate::app::App;
use crate::ui::style::make_block;

const TAB_TITLES: [&str; 3] = ["Detail", "Raw Log", "Metadata"];

pub fn draw_detail(frame: &mut Frame, app: &App, area: Rect, focused: bool) {
    let block = make_block("Detail", focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 || inner.width < 4 {
        return;
    }

    // Split: 1-line tabs bar + rest for content.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    // ── Tabs ──────────────────────────────────────────────────────────
    let titles: Vec<Line> = TAB_TITLES
        .iter()
        .enumerate()
        .map(|(i, t)| {
            if i == app.detail_tab {
                Line::from(Span::styled(
                    *t,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(Span::styled(*t, Style::default().fg(Color::DarkGray)))
            }
        })
        .collect();

    let tabs = Tabs::new(titles)
        .select(app.detail_tab)
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::raw(" │ "));

    frame.render_widget(tabs, chunks[0]);

    // ── Tab content ───────────────────────────────────────────────────
    let content: Vec<Line> = match app.detail_tab {
        0 => detail_tab_content(app),
        1 => raw_log_tab_content(app),
        2 => metadata_tab_content(app),
        _ => vec![],
    };

    let paragraph = Paragraph::new(content)
        .wrap(Wrap { trim: false })
        .scroll((app.detail_scroll, 0));

    frame.render_widget(paragraph, chunks[1]);
}

fn detail_tab_content(app: &App) -> Vec<Line<'static>> {
    let Some(err) = app.selected_error() else {
        return vec![Line::from(Span::styled(
            "Select an error to view details",
            Style::default().fg(Color::DarkGray),
        ))];
    };

    let mut lines = Vec::new();

    lines.push(Line::from(vec![
        Span::styled("Rule:     ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(err.rule.clone()),
    ]));

    lines.push(Line::from(vec![
        Span::styled("Severity: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(err.severity.to_string()),
    ]));

    lines.push(Line::from(vec![
        Span::styled("Location: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(format!("{}:{}", err.line, err.col)),
    ]));

    if let Some(ref path) = err.json_path {
        lines.push(Line::from(vec![
            Span::styled("Path:     ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(path.clone()),
        ]));
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "Message:",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::raw(err.message.clone()));

    lines
}

fn raw_log_tab_content(app: &App) -> Vec<Line<'static>> {
    let log = app.current_phase_log();
    if log.is_empty() {
        return vec![Line::from(Span::styled(
            "No log available",
            Style::default().fg(Color::DarkGray),
        ))];
    }
    log.lines().map(|l| Line::raw(l.to_string())).collect()
}

fn metadata_tab_content(app: &App) -> Vec<Line<'static>> {
    let Some(report) = &app.report else {
        return vec![Line::from(Span::styled(
            "No report loaded",
            Style::default().fg(Color::DarkGray),
        ))];
    };

    vec![
        Line::from(vec![
            Span::styled("Spec:    ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(report.spec.clone()),
        ]),
        Line::from(vec![
            Span::styled("Mode:    ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(report.mode.clone()),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("Total:   ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(report.summary.total.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Passed:  ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(report.summary.passed.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Failed:  ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(report.summary.failed.to_string()),
        ]),
    ]
}
