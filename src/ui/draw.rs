use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, BrowserPanel, Panel, ScreenMode, StatusLevel, ViewMode};

use super::overlay;
use super::panels;

pub fn draw(frame: &mut Frame, app: &App) {
    let size = frame.area();

    // Reserve 1 line at the bottom for the status bar.
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(size);

    match app.view_mode {
        ViewMode::Validator => draw_panels(frame, app, outer[0]),
        ViewMode::CodeBrowser => panels::draw_code_browser(frame, app, outer[0]),
    }
    draw_bottom_bar(frame, app, outer[1]);

    if app.view_mode == ViewMode::Validator {
        if let Some(ref proposal) = app.fix_proposal {
            overlay::draw_fix_overlay(frame, proposal, size);
        } else if app.show_help {
            overlay::draw_help_overlay(frame, size);
        }
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
    // Spinner occupies fixed width on the right when validating.
    const SPINNER_WIDTH: u16 = 16; // " ⠋ Validating "
    let spinner_len = if app.validating { SPINNER_WIDTH } else { 0 };

    let bar_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(spinner_len)])
        .split(area);

    // ── Left side: status message or context-sensitive hints ──
    let left_spans = if let Some(msg) = &app.status_message {
        let color = match msg.level {
            StatusLevel::Info => Color::Cyan,
            StatusLevel::Warn => Color::Yellow,
            StatusLevel::Error => Color::Red,
        };
        let mut spans = vec![
            Span::styled(&msg.text, Style::default().fg(color)),
            Span::raw("  "),
        ];
        if app.validating {
            push_hint_spans(&mut spans, "Esc", "cancel");
            spans.push(Span::raw("  "));
        }
        push_hint_spans(&mut spans, "?", "help");
        spans
    } else {
        let mut hints: Vec<(&str, &str)> = if app.view_mode == ViewMode::CodeBrowser {
            match app.browser.browser_focus {
                BrowserPanel::FileTree => vec![
                    ("j/k", "navigate"),
                    ("Enter", "open"),
                    ("[/]", "generator"),
                    ("Tab", "panel"),
                    ("g", "validator"),
                ],
                BrowserPanel::FileContent => {
                    vec![("j/k", "scroll"), ("Tab", "panel"), ("g", "validator")]
                }
            }
        } else {
            match app.focused_panel {
                Panel::Phases => vec![
                    ("j/k", "navigate"),
                    ("Enter", "select"),
                    ("r", "run"),
                    ("g", "browser"),
                ],
                Panel::Errors => vec![
                    ("j/k", "navigate"),
                    ("Enter/d", "detail"),
                    ("e", "edit"),
                    ("f", "fix"),
                    ("r", "run"),
                ],
                Panel::Detail => vec![("j/k", "scroll"), ("[/]", "tab")],
                Panel::SpecContext => vec![("j/k", "scroll")],
            }
        };
        if app.validating {
            hints.push(("Esc", "cancel"));
        }
        hints.push(("?", "help"));

        let mut spans = Vec::new();
        for (i, (key, action)) in hints.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw("  "));
            }
            push_hint_spans(&mut spans, key, action);
        }
        spans
    };
    frame.render_widget(Paragraph::new(Line::from(left_spans)), bar_layout[0]);

    // ── Right side: spinner when validating ──
    if app.validating {
        const BRAILLE: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let frame_char = BRAILLE[app.tick / 3 % BRAILLE.len()];
        let spinner = Line::from(Span::styled(
            format!(" {frame_char} Validating "),
            Style::default().fg(Color::Yellow),
        ));
        frame.render_widget(
            Paragraph::new(spinner).alignment(ratatui::layout::Alignment::Right),
            bar_layout[1],
        );
    }
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
