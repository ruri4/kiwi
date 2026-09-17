use crate::state::AppState;
use crate::types::Role;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Wrap};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const INK: Color = Color::Rgb(10, 11, 16);
const FOG: Color = Color::Rgb(214, 216, 226);
const MUTED: Color = Color::Rgb(125, 128, 143);
const VIOLET: Color = Color::Rgb(169, 139, 250);
const CYAN: Color = Color::Rgb(82, 203, 231);
const AMBER: Color = Color::Rgb(238, 183, 91);
const USER_BG: Color = Color::Rgb(18, 43, 53);
const ASSISTANT_BG: Color = Color::Rgb(37, 30, 57);
const REASONING_BG: Color = Color::Rgb(24, 27, 36);
const ERROR_BG: Color = Color::Rgb(57, 31, 35);

pub fn draw(frame: &mut Frame<'_>, state: &mut AppState) {
    let area = frame.area();
    frame.render_widget(Block::default().style(Style::default().bg(INK)), area);
    let [header, transcript, composer, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(3),
        Constraint::Length(7),
        Constraint::Length(3),
    ])
    .areas(area);

    draw_header(frame, state, header);
    draw_transcript(frame, state, transcript);
    draw_composer(frame, state, composer);
    draw_footer(frame, footer);
}

fn draw_header(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let status_style = Style::default()
        .fg(AMBER)
        .add_modifier(if state.status_is_error() {
            Modifier::BOLD
        } else {
            Modifier::empty()
        });
    let line = Line::from(vec![
        Span::styled("model ", Style::default().fg(MUTED)),
        Span::styled(state.model().to_owned(), Style::default().fg(VIOLET)),
        Span::styled("  reasoning ", Style::default().fg(MUTED)),
        Span::styled(
            state.reasoning_effort().to_string(),
            Style::default().fg(CYAN),
        ),
        Span::styled("  status ", Style::default().fg(MUTED)),
        Span::styled(state.status().to_owned(), status_style),
    ]);
    let block = Block::default()
        .padding(Padding::new(2, 2, 1, 1))
        .style(Style::default().bg(INK));
    frame.render_widget(
        Paragraph::new(line)
            .style(Style::default().bg(INK))
            .block(block),
        area,
    );
}

fn draw_transcript(frame: &mut Frame<'_>, state: &mut AppState, area: Rect) {
    let block = Block::default()
        .padding(Padding::new(2, 2, 1, 1))
        .style(Style::default().bg(INK));
    let inner = block.inner(area);
    let width = usize::from(inner.width.max(1));
    let horizontal_padding = 2.min(width / 2);
    let content_width = width.saturating_sub(horizontal_padding * 2).max(1);
    let mut lines = Vec::new();
    let mut rows = 0usize;
    for message in &state.transcript {
        let content_style = message_style(message.role);
        lines.push(padded_message_line(
            "",
            width,
            horizontal_padding,
            content_style,
        ));
        rows += 1;
        for content_line in message.content.split('\n') {
            for wrapped_line in wrap_line(content_line, content_width) {
                let line_width = UnicodeWidthStr::width(wrapped_line.as_str());
                lines.push(padded_message_line(
                    &wrapped_line,
                    width,
                    horizontal_padding,
                    content_style,
                ));
                rows += line_width.max(1).div_ceil(content_width).max(1);
            }
        }
        lines.push(padded_message_line(
            "",
            width,
            horizontal_padding,
            content_style,
        ));
        rows += 1;
    }

    let page = usize::from(inner.height.max(1));
    state.transcript_page = page;
    let max_scroll = rows.saturating_sub(page);
    state.scroll_from_bottom = state.scroll_from_bottom.min(max_scroll);
    let scroll = max_scroll.saturating_sub(state.scroll_from_bottom);
    let scroll = saturating_u16(scroll);
    let transcript = Paragraph::new(Text::from(lines))
        .style(Style::default().fg(FOG).bg(INK))
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(transcript, area);
}

fn message_style(role: Role) -> Style {
    let (foreground, background) = match role {
        Role::User => (CYAN, USER_BG),
        Role::Assistant => (FOG, ASSISTANT_BG),
        Role::Reasoning => (MUTED, REASONING_BG),
        Role::Error => (AMBER, ERROR_BG),
    };
    Style::default().fg(foreground).bg(background)
}

fn padded_message_line(
    content: &str,
    width: usize,
    horizontal_padding: usize,
    style: Style,
) -> Line<'static> {
    let content_width = UnicodeWidthStr::width(content);
    let right_padding = width.saturating_sub(horizontal_padding + content_width);
    let line = format!(
        "{}{}{}",
        " ".repeat(horizontal_padding),
        content,
        " ".repeat(right_padding),
    );
    Line::from(Span::styled(line, style))
}

fn wrap_line(content: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut line_width = 0usize;

    for character in content.chars() {
        let character_width = character.width().unwrap_or(0);
        if character_width > 0 && line_width + character_width > width && !line.is_empty() {
            lines.push(std::mem::take(&mut line));
            line_width = 0;
        }
        line.push(character);
        line_width += character_width;
        if line_width >= width {
            lines.push(std::mem::take(&mut line));
            line_width = 0;
        }
    }

    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    lines
}

fn draw_composer(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let area = area.inner(Margin {
        horizontal: 2,
        vertical: 0,
    });
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(MUTED))
        .padding(Padding::new(1, 1, 0, 0))
        .style(Style::default().bg(INK));
    let inner = block.inner(area);
    let width = usize::from(inner.width.max(1));
    let (cursor_row, cursor_column) =
        visual_cursor(state.composer.text(), state.composer.cursor(), width);
    let visible_rows = usize::from(inner.height.max(1));
    let vertical_scroll = cursor_row.saturating_sub(visible_rows.saturating_sub(1));
    let vertical_scroll_u16 = saturating_u16(vertical_scroll);
    let paragraph = Paragraph::new(state.composer.text().to_owned())
        .style(Style::default().fg(FOG).bg(INK))
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((vertical_scroll_u16, 0));
    frame.render_widget(paragraph, area);

    if inner.width > 0 && inner.height > 0 {
        let x_offset = saturating_u16(cursor_column.min(width.saturating_sub(1)));
        let visible_row = cursor_row.saturating_sub(vertical_scroll);
        let y_offset = saturating_u16(visible_row.min(visible_rows.saturating_sub(1)));
        frame.set_cursor_position(Position::new(
            inner.x.saturating_add(x_offset),
            inner.y.saturating_add(y_offset),
        ));
    }
}

fn visual_cursor(text: &str, cursor: usize, width: usize) -> (usize, usize) {
    let mut row = 0usize;
    let mut column = 0usize;
    let Some(before_cursor) = text.get(..cursor) else {
        return (0, 0);
    };
    for character in before_cursor.chars() {
        if character == '\n' {
            row += 1;
            column = 0;
            continue;
        }
        let character_width = character.width().unwrap_or(0);
        if column + character_width > width {
            row += 1;
            column = 0;
        }
        column += character_width;
        if column >= width {
            row += 1;
            column = 0;
        }
    }
    (row, column)
}

fn saturating_u16(value: usize) -> u16 {
    value.min(usize::from(u16::MAX)) as u16
}

fn draw_footer(frame: &mut Frame<'_>, area: Rect) {
    let block = Block::default()
        .padding(Padding::new(2, 2, 1, 1))
        .style(Style::default().bg(INK));
    let footer = Paragraph::new(
        "enter send  alt+enter newline  pgup/pgdn scroll  ctrl+r effort  ctrl+c/q quit",
    )
    .style(Style::default().fg(MUTED).bg(INK))
    .block(block);
    frame.render_widget(footer, area);
}
