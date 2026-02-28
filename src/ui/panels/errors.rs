use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState};

use crate::app::App;
use crate::ui::style::{COLOR_SELECTED_BG, ICON_SEVERITY, make_block, severity_color};

pub fn draw_errors(frame: &mut Frame, app: &App, area: Rect, focused: bool) {
    let block = make_block("Errors", focused);
    let errors = app.current_errors();

    if errors.is_empty() {
        let msg = if app.report.is_some() {
            "No errors in this phase"
        } else {
            "No data"
        };
        let item = ListItem::new(Line::from(Span::styled(
            msg,
            Style::default().fg(ratatui::style::Color::DarkGray),
        )));
        let list = List::new(vec![item]).block(block);
        frame.render_widget(list, area);
        return;
    }

    // Compute available width inside the block borders.
    let inner_width = area.width.saturating_sub(2) as usize;

    let items: Vec<ListItem> = errors
        .iter()
        .enumerate()
        .map(|(i, err)| {
            let sev_color = severity_color(err.severity);

            // Truncate rule to ~20 chars.
            let rule_display: String = if err.rule.len() > 20 {
                format!("{}…", &err.rule[..19])
            } else {
                err.rule.clone()
            };

            // "● rule_id  " takes up prefix_len chars.
            let prefix_len = 2 + rule_display.len() + 2; // icon+space + rule + 2 spaces
            let msg_budget = inner_width.saturating_sub(prefix_len);
            let msg_display: String = if err.message.len() > msg_budget && msg_budget > 1 {
                format!("{}…", &err.message[..msg_budget.saturating_sub(1)])
            } else {
                err.message.clone()
            };

            let spans = vec![
                Span::styled(format!("{ICON_SEVERITY} "), Style::default().fg(sev_color)),
                Span::styled(rule_display, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("  "),
                Span::raw(msg_display),
            ];

            let mut style = Style::default();
            if focused && i == app.error_index {
                style = style.bg(COLOR_SELECTED_BG);
            }

            ListItem::new(Line::from(spans)).style(style)
        })
        .collect();

    let mut state = ListState::default();
    if focused {
        state.select(Some(app.error_index));
    }

    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(COLOR_SELECTED_BG)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(list, area, &mut state);
}
