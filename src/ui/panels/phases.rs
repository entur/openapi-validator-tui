use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState};

use crate::app::App;
use crate::ui::style::{
    COLOR_FAIL, COLOR_SELECTED_BG, make_block, phase_status_color, phase_status_icon,
};

pub fn draw_phases(frame: &mut Frame, app: &App, area: Rect, focused: bool) {
    let block = make_block("Phases", focused);
    let entries = app.phase_entries();

    if entries.is_empty() {
        let item = ListItem::new(Line::from(Span::styled(
            "No validation report loaded",
            Style::default().fg(ratatui::style::Color::DarkGray),
        )));
        let list = List::new(vec![item]).block(block);
        frame.render_widget(list, area);
        return;
    }

    let items: Vec<ListItem> = entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let icon_color = phase_status_color(entry.status);
            let icon = phase_status_icon(entry.status);

            let mut spans = vec![
                Span::styled(format!("{icon} "), Style::default().fg(icon_color)),
                Span::raw(&entry.label),
            ];

            if entry.error_count > 0 {
                spans.push(Span::styled(
                    format!(" [{} errors]", entry.error_count),
                    Style::default().fg(COLOR_FAIL),
                ));
            }

            let mut style = Style::default();
            if focused && i == app.phase_index {
                style = style.bg(COLOR_SELECTED_BG);
            }

            ListItem::new(Line::from(spans)).style(style)
        })
        .collect();

    let mut state = ListState::default();
    if focused {
        state.select(Some(app.phase_index));
    }

    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(COLOR_SELECTED_BG)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(list, area, &mut state);
}
