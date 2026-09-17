use crate::picker::PickerKind;
use crate::state::AppState;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

const PANEL: Color = Color::Rgb(20, 21, 29);
const FOG: Color = Color::Rgb(214, 216, 226);
const MUTED: Color = Color::Rgb(125, 128, 143);
const VIOLET: Color = Color::Rgb(169, 139, 250);
const CYAN: Color = Color::Rgb(82, 203, 231);

pub(super) fn draw(frame: &mut Frame<'_>, state: &AppState) {
    let Some(picker) = state.picker() else {
        return;
    };
    let area = centered(frame.area(), picker.items.len());
    if area.width < 4 || area.height < 4 {
        return;
    }
    frame.render_widget(Clear, area);
    let title = match picker.kind {
        PickerKind::Endpoint => " endpoints ",
        PickerKind::Model => " models ",
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(VIOLET))
        .style(Style::default().fg(FOG).bg(PANEL));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [list_area, help_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
    let visible = usize::from(list_area.height.max(1));
    let start = picker
        .selected
        .saturating_sub(visible / 2)
        .min(picker.items.len().saturating_sub(visible));
    let lines = picker
        .items
        .iter()
        .enumerate()
        .skip(start)
        .take(visible)
        .map(|(index, item)| {
            let selected = index == picker.selected;
            let style = if selected {
                Style::default()
                    .fg(CYAN)
                    .bg(Color::Rgb(37, 30, 57))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FOG).bg(PANEL)
            };
            Line::from(Span::styled(
                format!("{} {item}", if selected { '>' } else { ' ' }),
                style,
            ))
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(Text::from(lines)).style(Style::default().bg(PANEL)),
        list_area,
    );
    frame.render_widget(
        Paragraph::new("↑/↓ select  enter use  esc close")
            .style(Style::default().fg(MUTED).bg(PANEL)),
        help_area,
    );
}

fn centered(area: Rect, item_count: usize) -> Rect {
    let width = area.width.saturating_sub(2).min(76);
    let wanted_height = item_count.saturating_add(3).min(22) as u16;
    let height = area.height.saturating_sub(2).min(wanted_height.max(4));
    Rect::new(
        area.x.saturating_add(area.width.saturating_sub(width) / 2),
        area.y
            .saturating_add(area.height.saturating_sub(height) / 2),
        width,
        height,
    )
}
