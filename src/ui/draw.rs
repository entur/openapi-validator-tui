use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, Panel, ScreenMode, StatusLevel};

use super::overlay;
use super::panels;

pub fn draw(frame: &mut Frame, app: &App) {
    let size = frame.area();

    // Reserve 1 line at the bottom for the status bar.
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(size);

    draw_panels(frame, app, outer[0]);
    draw_bottom_bar(frame, app, outer[1]);

    if app.show_help {
        overlay::draw_help_overlay(frame, size);
    }
}

fn draw_panels(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    // Outer split: left panels | right panels.
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(match app.screen_mode {
            ScreenMode::Normal => vec![Constraint::Percentage(30), Constraint::Percentage(70)],
            ScreenMode::Half => vec![Constraint::Percentage(20), Constraint::Percentage(80)],
            ScreenMode::Full => vec![Constraint::Percentage(0), Constraint::Percentage(100)],
        })
        .split(area);

    // Left column: phases (top) + errors (bottom).
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(horizontal[0]);

    // Right column: detail (top) + spec context (bottom).
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(horizontal[1]);

    panels::draw_phases(frame, app, left[0], app.focused_panel == Panel::Phases);
    panels::draw_errors(frame, app, left[1], app.focused_panel == Panel::Errors);
    panels::draw_detail(frame, app, right[0], app.focused_panel == Panel::Detail);
    panels::draw_spec_context(
        frame,
        app,
        right[1],
        app.focused_panel == Panel::SpecContext,
    );
}

fn draw_bottom_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    // 1. Status message takes priority.
    if let Some(msg) = &app.status_message {
        let color = match msg.level {
            StatusLevel::Info => Color::Cyan,
            StatusLevel::Warn => Color::Yellow,
            StatusLevel::Error => Color::Red,
        };
        let mut spans = vec![
            Span::styled(&msg.text, Style::default().fg(color)),
            Span::raw("  "),
        ];
        push_hint_spans(&mut spans, "?", "help");
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
        return;
    }

    // 2. Validating state.
    if app.validating {
        let mut spans = vec![
            Span::styled("Validating...", Style::default().fg(Color::Yellow)),
            Span::raw("  "),
        ];
        push_hint_spans(&mut spans, "Esc", "cancel");
        spans.push(Span::raw("  "));
        push_hint_spans(&mut spans, "?", "help");
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
        return;
    }

    // 3. Normal: context-sensitive hints.
    let hints: &[(&str, &str)] = match app.focused_panel {
        Panel::Phases => &[
            ("j/k", "navigate"),
            ("Enter", "select"),
            ("r", "run"),
            ("?", "help"),
        ],
        Panel::Errors => &[
            ("j/k", "navigate"),
            ("Enter/d", "detail"),
            ("e", "edit"),
            ("r", "run"),
            ("?", "help"),
        ],
        Panel::Detail => &[("j/k", "scroll"), ("[/]", "tab"), ("?", "help")],
        Panel::SpecContext => &[("j/k", "scroll"), ("?", "help")],
    };

    let mut spans = Vec::new();
    for (i, (key, action)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        push_hint_spans(&mut spans, key, action);
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn push_hint_spans<'a>(spans: &mut Vec<Span<'a>>, key: &'a str, action: &'a str) {
    spans.push(Span::styled(
        format!("[{key}]"),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::styled(
        format!(" {action}"),
        Style::default().fg(Color::DarkGray),
    ));
}
