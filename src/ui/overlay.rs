use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::fix::FixProposal;

/// Draw the help overlay centered on the screen.
pub fn draw_help_overlay(frame: &mut Frame, area: Rect) {
    let popup = centered_rect(62, 25, area);

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
        ("Ctrl+U/D", Some("Half-page (detail/spec)")),
        ("Tab / l", Some("Next panel")),
        ("S-Tab / h", Some("Previous panel")),
        ("1-4", Some("Jump to panel")),
    ]);

    let action_lines = keybinding_lines(&[
        ("Actions", None),
        ("Enter", Some("Select / focus next")),
        ("d", Some("Jump to detail")),
        ("e", Some("Open in $EDITOR")),
        ("f", Some("Propose fix")),
        ("r", Some("Run validation")),
        ("Esc", Some("Cancel validation")),
        ("+", Some("Expand layout")),
        ("_", Some("Shrink layout")),
        ("[/]", Some("Switch detail tab")),
        ("q", Some("Quit")),
        ("?", Some("Toggle this help")),
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
        Paragraph::new(dismiss).alignment(Alignment::Center),
        dismiss_area,
    );
}

/// Draw the fix proposal overlay centered on the screen.
pub fn draw_fix_overlay(frame: &mut Frame, proposal: &FixProposal, area: Rect) {
    let content_lines = build_fix_lines(proposal);
    // Height: border(2) + content lines + 1 blank + keybindings line.
    let height = (content_lines.len() as u16) + 4;
    let popup = centered_rect(70, height, area);

    frame.render_widget(Clear, popup);

    let title = format!(" Fix: {} ", proposal.rule);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title(title);

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    // Content area (everything except the last line for keybindings).
    let content_area = Rect {
        height: inner.height.saturating_sub(2),
        ..inner
    };
    frame.render_widget(Paragraph::new(content_lines), content_area);

    // Keybindings hint at the bottom.
    let hint_line = Line::from(vec![
        Span::styled(
            "[y]",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" accept  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "[n]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" skip  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "[Esc]",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" cancel", Style::default().fg(Color::DarkGray)),
    ]);

    let hint_area = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(1),
        width: inner.width,
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(vec![hint_line]).alignment(Alignment::Center),
        hint_area,
    );
}

fn build_fix_lines(proposal: &FixProposal) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let dim = Style::default().fg(Color::DarkGray);
    let green = Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD);

    // Description.
    lines.push(Line::from(Span::styled(
        proposal.description.clone(),
        Style::default().fg(Color::White),
    )));
    lines.push(Line::from(""));

    // Context before.
    let ctx_start = proposal
        .target_line
        .saturating_sub(proposal.context_before.len());
    for (i, line) in proposal.context_before.iter().enumerate() {
        let line_num = ctx_start + i + 1;
        lines.push(Line::from(vec![
            Span::styled(format!("  {line_num:>4} │ "), dim),
            Span::styled(line.clone(), dim),
        ]));
    }

    // Inserted lines (green, with + prefix).
    for (i, line) in proposal.inserted.iter().enumerate() {
        let line_num = proposal.target_line + i + 1;
        lines.push(Line::from(vec![
            Span::styled(format!("+ {line_num:>4} │ "), green),
            Span::styled(line.clone(), green),
        ]));
    }

    // Context after.
    let after_start = proposal.target_line + proposal.inserted.len() + 1;
    for (i, line) in proposal.context_after.iter().enumerate() {
        let line_num = after_start + i;
        lines.push(Line::from(vec![
            Span::styled(format!("  {line_num:>4} │ "), dim),
            Span::styled(line.clone(), dim),
        ]));
    }

    lines
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
