use crate::session::BrowserMode;
use crate::state::AppState;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use unicode_width::UnicodeWidthStr;

const PANEL: Color = Color::Rgb(20, 21, 29);
const FOG: Color = Color::Rgb(214, 216, 226);
const MUTED: Color = Color::Rgb(125, 128, 143);
const VIOLET: Color = Color::Rgb(169, 139, 250);
const CYAN: Color = Color::Rgb(82, 203, 231);
const AMBER: Color = Color::Rgb(238, 183, 91);

pub(super) fn draw(frame: &mut Frame<'_>, state: &AppState) {
    let Some(browser) = state.session_browser() else {
        return;
    };
    let area = centered(frame.area());
    if area.width < 4 || area.height < 4 {
        return;
    }
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(" sessions ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(VIOLET))
        .style(Style::default().fg(FOG).bg(PANEL));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [list_area, help_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(inner);

    let sessions = state.sessions();
    let visible_count = usize::from(list_area.height.max(1));
    let max_start = sessions.len().saturating_sub(visible_count);
    let start = browser
        .selected
        .saturating_sub(visible_count / 2)
        .min(max_start);
    let end = start.saturating_add(visible_count).min(sessions.len());
    let mut lines = Vec::new();
    let mut rename_cursor = None;

    for (index, session) in sessions.iter().enumerate().take(end).skip(start) {
        let selected = index == browser.selected;
        let active = session.id == state.active_session_id();
        let marker = match (selected, active) {
            (true, true) => "> * ",
            (true, false) => ">   ",
            (false, true) => "  * ",
            (false, false) => "    ",
        };
        let style = if selected {
            Style::default()
                .fg(CYAN)
                .bg(Color::Rgb(37, 30, 57))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(FOG).bg(PANEL)
        };
        if selected && let BrowserMode::Rename(editor) = &browser.mode {
            let text = format!("{marker}{}", editor.text());
            let before_cursor = editor.text().get(..editor.cursor()).unwrap_or("");
            rename_cursor = Some((
                lines.len(),
                UnicodeWidthStr::width(marker) + UnicodeWidthStr::width(before_cursor),
            ));
            lines.push(Line::from(Span::styled(text, style)));
        } else {
            lines.push(Line::from(Span::styled(
                format!("{marker}{}", session.title),
                style,
            )));
        }
    }
    frame.render_widget(
        Paragraph::new(Text::from(lines)).style(Style::default().bg(PANEL)),
        list_area,
    );

    let help = match &browser.mode {
        BrowserMode::Browse => "↑/↓ select  enter switch  n new  r rename  d delete  esc close",
        BrowserMode::Rename(_) => "rename: enter save (nonblank)  esc cancel",
        BrowserMode::ConfirmDelete => "delete selected session?  y confirm  n/esc cancel",
    };
    let help_style = if matches!(&browser.mode, BrowserMode::ConfirmDelete) {
        Style::default().fg(AMBER).bg(PANEL)
    } else {
        Style::default().fg(MUTED).bg(PANEL)
    };
    frame.render_widget(Paragraph::new(help).style(help_style), help_area);

    if let Some((row, column)) = rename_cursor {
        let x = list_area
            .x
            .saturating_add(column.min(usize::from(list_area.width.saturating_sub(1))) as u16);
        let y = list_area.y.saturating_add(row as u16);
        if y < list_area.bottom() {
            frame.set_cursor_position(Position::new(x, y));
        }
    }
}

fn centered(area: Rect) -> Rect {
    let width = area.width.saturating_sub(2).min(76);
    let height = area.height.saturating_sub(2).min(22);
    Rect::new(
        area.x.saturating_add(area.width.saturating_sub(width) / 2),
        area.y
            .saturating_add(area.height.saturating_sub(height) / 2),
        width,
        height,
    )
}
