use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

/// Draw the help overlay centered on the screen.
pub fn draw_help_overlay(frame: &mut Frame, area: Rect) {
    let popup = centered_rect(62, 24, area);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Keybindings ");

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    // Two-column layout.
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    let nav_lines = keybinding_lines(&[
        ("Navigation", None),
        ("j / ↓", Some("Move down")),
        ("k / ↑", Some("Move up")),
        ("Home / <", Some("Jump to first")),
        ("End / >", Some("Jump to last")),
        ("PgUp", Some("Page up")),
        ("PgDn", Some("Page down")),
        ("Ctrl+U/D", Some("Half-page scroll")),
        ("Tab / l", Some("Next panel")),
        ("S-Tab / h", Some("Previous panel")),
        ("1-4", Some("Jump to panel")),
    ]);

    let action_lines = keybinding_lines(&[
        ("Actions", None),
        ("Enter", Some("Select / focus next")),
        ("d", Some("Jump to detail")),
        ("r", Some("Run validation")),
        ("Esc", Some("Cancel validation")),
        ("+", Some("Expand layout")),
        ("_", Some("Shrink layout")),
        ("[/]", Some("Switch detail tab")),
        ("q", Some("Quit")),
        ("?", Some("Toggle this help")),
        ("", None),
    ]);

    let dismiss = vec![Line::from(Span::styled(
        "Press any key to close",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
    ))];

    // Navigation column.
    let nav_area = Rect {
        height: columns[0].height.saturating_sub(1),
        ..columns[0]
    };
    frame.render_widget(Paragraph::new(nav_lines), nav_area);

    // Actions column.
    let act_area = Rect {
        height: columns[1].height.saturating_sub(1),
        ..columns[1]
    };
    frame.render_widget(Paragraph::new(action_lines), act_area);

    // Dismiss hint at the bottom.
    let dismiss_area = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(1),
        width: inner.width,
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(dismiss).alignment(ratatui::layout::Alignment::Center),
        dismiss_area,
    );
}

fn keybinding_lines(items: &[(&str, Option<&str>)]) -> Vec<Line<'static>> {
    items
        .iter()
        .map(|(key, action)| match action {
            None => Line::from(Span::styled(
                key.to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Some(desc) => Line::from(vec![
                Span::styled(
                    format!("  {key:<14}"),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(desc.to_string(), Style::default().fg(Color::White)),
            ]),
        })
        .collect()
}

/// Return a centered `Rect` of the given fixed size within `area`.
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    }
}
