use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use unicode_width::UnicodeWidthChar;

use crate::app::{App, Mode};
use crate::diff::DiffTag;
use crate::highlight;

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // status bar
            Constraint::Min(1),   // editor
            Constraint::Length(1), // hint bar / message
        ])
        .split(area);

    // --- Status bar ---
    let status = app.status_line();
    let status_style = Style::default().bg(Color::DarkGray).fg(Color::White);
    let status_bar = Paragraph::new(Line::from(Span::styled(
        format!(" {}", status),
        status_style,
    )))
    .style(status_style);
    frame.render_widget(status_bar, chunks[0]);

    // --- Main area ---
    let area_height = chunks[1].height as usize;

    match app.mode {
        Mode::Edit => {
            let lines: Vec<Line> = app
                .buffer
                .visible_lines(area_height)
                .map(|(_, text)| highlight::highlight_line(&text))
                .collect();
            let editor = Paragraph::new(lines).block(Block::default());
            frame.render_widget(editor, chunks[1]);

            // Cursor — use display width for correct CJK positioning
            let cursor_y = chunks[1].y + (app.buffer.cursor.line - app.buffer.scroll_offset) as u16;
            let line_slice = app.buffer.rope.line(app.buffer.cursor.line);
            let display_col: usize = line_slice
                .chars()
                .take(app.buffer.cursor.col)
                .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
                .sum();
            let cursor_x = chunks[1].x + display_col as u16;
            frame.set_cursor_position((cursor_x, cursor_y));
        }
        Mode::Preview => {
            let all_lines: Vec<Line> = render_preview(&app.buffer.rope.to_string());
            let total = all_lines.len();
            let offset = app.preview_scroll.min(total.saturating_sub(1));
            let visible: Vec<Line> = all_lines
                .into_iter()
                .skip(offset)
                .take(area_height)
                .collect();
            let preview = Paragraph::new(visible)
                .block(Block::default().title(" PREVIEW (Esc to exit) ").borders(Borders::TOP));
            frame.render_widget(preview, chunks[1]);
        }
        Mode::Outline => {
            let lines: Vec<Line> = app
                .outline_entries
                .iter()
                .enumerate()
                .map(|(i, entry)| {
                    let indent = "  ".repeat(entry.level.saturating_sub(1));
                    let marker = if i == app.outline_selected { "> " } else { "  " };
                    let text = format!("{}{}{}", marker, indent, entry.text);
                    let style = if i == app.outline_selected {
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                    } else {
                        match entry.level {
                            1 => Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                            2 => Style::default().fg(Color::Cyan),
                            3 => Style::default().fg(Color::Blue),
                            _ => Style::default().fg(Color::DarkGray),
                        }
                    };
                    Line::from(Span::styled(text, style))
                })
                .collect();
            let outline = Paragraph::new(lines)
                .block(Block::default().title(" OUTLINE (Enter jump, Esc back) ").borders(Borders::TOP));
            frame.render_widget(outline, chunks[1]);
        }
        Mode::Diff => {
            let total = app.diff_lines.len();
            let offset = app.diff_scroll.min(total.saturating_sub(1));
            let lines: Vec<Line> = app
                .diff_lines
                .iter()
                .skip(offset)
                .take(area_height)
                .map(|dl| {
                    let (prefix, style) = match dl.tag {
                        DiffTag::Equal => (
                            "  ",
                            Style::default().fg(Color::DarkGray),
                        ),
                        DiffTag::Insert => (
                            "+ ",
                            Style::default().fg(Color::Green),
                        ),
                        DiffTag::Delete => (
                            "- ",
                            Style::default().fg(Color::Red),
                        ),
                    };
                    Line::from(Span::styled(
                        format!("{}{}", prefix, dl.text),
                        style,
                    ))
                })
                .collect();
            let diff_widget = Paragraph::new(lines)
                .block(Block::default().title(" DIFF — External change detected ").borders(Borders::TOP));
            frame.render_widget(diff_widget, chunks[1]);
        }
    }

    // --- Hint bar / message ---
    let hint_text = if let Some(ref msg) = app.message {
        Span::styled(
            format!(" {}", msg),
            Style::default().fg(Color::Yellow),
        )
    } else {
        let hints = match app.mode {
            Mode::Edit if app.external_change => " Alt+D Diff | Alt+S Save | Alt+P Preview | Alt+Q Quit",
            Mode::Edit => " Alt+S Save | Alt+Z Undo | Alt+P Preview | Alt+O Outline | Alt+Q Quit",
            Mode::Preview => " Esc Back | ↑↓ Scroll",
            Mode::Outline => " ↑↓ Navigate | Enter Jump | Esc Back",
            Mode::Diff => " A Accept | R Reject | E Edit | Esc Later | ↑↓ Scroll",
        };
        Span::styled(hints, Style::default().fg(Color::DarkGray))
    };
    let hint_bar =
        Paragraph::new(Line::from(hint_text)).style(Style::default().bg(Color::Black));
    frame.render_widget(hint_bar, chunks[2]);
}

fn render_preview(content: &str) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut in_code_block = false;

    for raw_line in content.lines() {
        let trimmed = raw_line.trim();

        // Toggle code block
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code_block = !in_code_block;
            if in_code_block {
                lines.push(Line::from(Span::styled(
                    " \u{2500}\u{2500}\u{2500} code \u{2500}\u{2500}\u{2500}".to_string(),
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}".to_string(),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            continue;
        }

        if in_code_block {
            lines.push(Line::from(Span::styled(
                format!("  {}", raw_line),
                Style::default().fg(Color::White).bg(Color::Rgb(30, 30, 30)),
            )));
            continue;
        }

        // Heading
        if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|&c| c == '#').count();
            let text = trimmed.trim_start_matches('#').trim();
            let (color, prefix) = match level {
                1 => (Color::Magenta, "\u{2588} "),
                2 => (Color::Cyan, "\u{2590} "),
                3 => (Color::Blue, "  \u{25b8} "),
                _ => (Color::DarkGray, "    "),
            };
            lines.push(Line::from(Span::styled(
                format!("{}{}", prefix, text),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )));
            if level <= 2 {
                lines.push(Line::from(""));
            }
            continue;
        }

        // Blockquote
        if trimmed.starts_with('>') {
            let text = trimmed.trim_start_matches('>').trim();
            lines.push(Line::from(Span::styled(
                format!("  \u{2502} {}", text),
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            )));
            continue;
        }

        // Horizontal rule
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            lines.push(Line::from(Span::styled(
                "\u{2500}".repeat(40),
                Style::default().fg(Color::DarkGray),
            )));
            continue;
        }

        // List items
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
            let text = &trimmed[2..];
            let indent = raw_line.len() - raw_line.trim_start().len();
            let pad = " ".repeat(indent);
            lines.push(Line::from(vec![
                Span::raw(format!("{}  ", pad)),
                Span::styled("\u{2022} ".to_string(), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(text.to_string()),
            ]));
            continue;
        }

        // Empty line
        if trimmed.is_empty() {
            lines.push(Line::from(""));
            continue;
        }

        // Regular text with indent
        lines.push(Line::from(format!("  {}", raw_line)));
    }

    lines
}
