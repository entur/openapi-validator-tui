use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders};

use crate::app::PhaseStatus;
use crate::log_parser::Severity;

// ── Colour constants ──────────────────────────────────────────────────
pub const COLOR_PASS: Color = Color::Green;
pub const COLOR_FAIL: Color = Color::Red;
pub const COLOR_RUNNING: Color = Color::Yellow;
pub const COLOR_PENDING: Color = Color::DarkGray;
pub const COLOR_SELECTED_BG: Color = Color::DarkGray;
pub const COLOR_GUTTER: Color = Color::DarkGray;

// ── Icon constants ────────────────────────────────────────────────────
pub const ICON_PASS: &str = "✓";
pub const ICON_FAIL: &str = "✗";
pub const ICON_RUNNING: &str = "◉";
pub const ICON_PENDING: &str = "─";

pub const ICON_SEVERITY: &str = "●";

// ── Helpers ───────────────────────────────────────────────────────────

pub fn severity_color(sev: Severity) -> Color {
    match sev {
        Severity::Error => Color::Red,
        Severity::Warning => Color::Yellow,
        Severity::Info => Color::Cyan,
        Severity::Hint => Color::DarkGray,
    }
}

pub fn phase_status_color(status: PhaseStatus) -> Color {
    match status {
        PhaseStatus::Pass => COLOR_PASS,
        PhaseStatus::Fail => COLOR_FAIL,
        PhaseStatus::Running => COLOR_RUNNING,
        PhaseStatus::Pending => COLOR_PENDING,
    }
}

pub fn phase_status_icon(status: PhaseStatus) -> &'static str {
    match status {
        PhaseStatus::Pass => ICON_PASS,
        PhaseStatus::Fail => ICON_FAIL,
        PhaseStatus::Running => ICON_RUNNING,
        PhaseStatus::Pending => ICON_PENDING,
    }
}

pub fn make_block(title: &str, focused: bool) -> Block<'_> {
    let style = if focused {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(style)
}
