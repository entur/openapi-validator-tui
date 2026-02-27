use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, Panel, ScreenMode};

pub fn draw(frame: &mut Frame, app: &App) {
    let size = frame.area();

    // Outer split: left panels | right panels
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(match app.screen_mode {
            ScreenMode::Normal => vec![Constraint::Percentage(30), Constraint::Percentage(70)],
            ScreenMode::Half => vec![Constraint::Percentage(20), Constraint::Percentage(80)],
            ScreenMode::Full => vec![Constraint::Percentage(0), Constraint::Percentage(100)],
        })
        .split(size);

    // Left column: phases (top) + errors (bottom)
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(horizontal[0]);

    // Right column: detail (top) + spec context (bottom)
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(horizontal[1]);

    let phases_block = make_block("Phases", app.focused_panel == Panel::Phases);
    let errors_block = make_block("Errors", app.focused_panel == Panel::Errors);
    let detail_block = make_block("Detail", app.focused_panel == Panel::Detail);
    let spec_block = make_block("Spec Context", app.focused_panel == Panel::SpecContext);

    frame.render_widget(
        Paragraph::new("No phases loaded").block(phases_block),
        left[0],
    );
    frame.render_widget(Paragraph::new("No errors").block(errors_block), left[1]);
    frame.render_widget(
        Paragraph::new("Select an error to view details").block(detail_block),
        right[0],
    );
    frame.render_widget(Paragraph::new("").block(spec_block), right[1]);
}

fn make_block(title: &str, focused: bool) -> Block<'_> {
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
