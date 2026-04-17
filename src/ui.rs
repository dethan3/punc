use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use unicode_width::UnicodeWidthChar;

use crate::app::{App, Mode, PreviewRow, PreviewRowKind, QuitAction};
use crate::diff::DiffTag;
use crate::highlight;

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // status bar
            Constraint::Min(1),    // editor
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

    match effective_mode(app) {
        Mode::Edit => {
            let area_width = chunks[1].width as usize;
            let scroll = app.buffer.scroll_offset;
            let total = app.buffer.rope.len_lines();
            let fetch_end = total.min(scroll + (area_height * 3).max(area_height + 50));
            // Pre-split each logical line into visual rows so wrapping works for
            // CJK and any text regardless of ratatui's word-wrap algorithm.
            let mut lines: Vec<Line> = Vec::new();
            for i in scroll..fetch_end {
                let text = app.buffer.line_text(i);
                for row in split_line_for_display(&text, area_width) {
                    lines.push(highlight::highlight_line(&row));
                }
            }
            let editor = Paragraph::new(lines).block(Block::default());
            frame.render_widget(editor, chunks[1]);

            // Cursor — account for soft-wrapped rows above the cursor line.
            let cursor_line = app
                .buffer
                .cursor
                .line
                .min(app.buffer.rope.len_lines().saturating_sub(1));
            let visual_rows_above: usize = (scroll..cursor_line)
                .map(|i| app.buffer.visual_rows_for_line(i, area_width))
                .sum();
            let display_col = app.buffer.cursor_display_col();
            let wrap_row = if area_width > 0 { display_col / area_width } else { 0 };
            let wrap_col = if area_width > 0 { display_col % area_width } else { display_col };
            let cursor_y = chunks[1].y + (visual_rows_above + wrap_row) as u16;
            let cursor_x = chunks[1].x + wrap_col as u16;
            frame.set_cursor_position((cursor_x, cursor_y));
        }
        Mode::Preview => {
            let total = app.preview_row_count();
            let offset = app.preview_scroll.min(total.saturating_sub(1));
            let visible_rows = app.preview_visible_rows(offset, area_height);
            let visible: Vec<Line> = visible_rows
                .iter()
                .map(|row| render_preview_row(app, row))
                .collect();
            let preview = Paragraph::new(visible).block(
                Block::default()
                    .title(" PREVIEW (Esc to exit) ")
                    .borders(Borders::TOP),
            );
            frame.render_widget(preview, chunks[1]);
        }
        Mode::Outline => {
            let lines: Vec<Line> = app
                .outline_entries
                .iter()
                .enumerate()
                .map(|(i, entry)| {
                    let indent = "  ".repeat(entry.level.saturating_sub(1));
                    let marker = if i == app.outline_selected {
                        "> "
                    } else {
                        "  "
                    };
                    let text = format!("{}{}{}", marker, indent, entry.text);
                    let style = if i == app.outline_selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        match entry.level {
                            1 => Style::default()
                                .fg(Color::Magenta)
                                .add_modifier(Modifier::BOLD),
                            2 => Style::default().fg(Color::Cyan),
                            3 => Style::default().fg(Color::Blue),
                            _ => Style::default().fg(Color::DarkGray),
                        }
                    };
                    Line::from(Span::styled(text, style))
                })
                .collect();
            let outline = Paragraph::new(lines).block(
                Block::default()
                    .title(" OUTLINE (Enter jump, Esc back) ")
                    .borders(Borders::TOP),
            );
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
                        DiffTag::Equal => ("  ", Style::default().fg(Color::DarkGray)),
                        DiffTag::Insert => ("+ ", Style::default().fg(Color::Green)),
                        DiffTag::Delete => ("- ", Style::default().fg(Color::Red)),
                    };
                    Line::from(Span::styled(format!("{}{}", prefix, dl.text), style))
                })
                .collect();
            let diff_widget = Paragraph::new(lines).block(
                Block::default()
                    .title(" DIFF — External change detected ")
                    .borders(Borders::TOP),
            );
            frame.render_widget(diff_widget, chunks[1]);
        }
        Mode::QuitConfirm => unreachable!(),
    }

    if app.mode == Mode::QuitConfirm {
        render_quit_confirm(frame, app);
    }

    // --- Hint bar / message ---
    let hint_text = if let Some(ref msg) = app.message {
        Span::styled(format!(" {}", msg), Style::default().fg(Color::Yellow))
    } else {
        let hints = match app.mode {
            Mode::Edit => {
                " Alt+D Diff | Alt+S Save | Alt+Z Undo | Alt+P Preview | Alt+O Outline | Alt+Q Quit"
            }
            Mode::Preview => " Esc Back | ↑↓ Scroll",
            Mode::Outline => " ↑↓ Navigate | Enter Jump | Esc Back",
            Mode::Diff => " A Accept | R Reject | E Edit | Esc Later | ↑↓ Scroll",
            Mode::QuitConfirm => " ←→ Select | Enter Confirm | S/D/Esc Direct",
        };
        Span::styled(hints, Style::default().fg(Color::DarkGray))
    };
    let hint_bar = Paragraph::new(Line::from(hint_text)).style(Style::default().bg(Color::Black));
    frame.render_widget(hint_bar, chunks[2]);
}

fn effective_mode(app: &App) -> Mode {
    match app.mode {
        Mode::QuitConfirm => app.quit_return_mode,
        mode => mode,
    }
}

fn render_quit_confirm(frame: &mut Frame, app: &App) {
    let popup = centered_rect(frame.area(), 56, 5);
    frame.render_widget(Clear, popup);

    let mut lines = vec![
        Line::from(Span::styled(
            "Unsaved changes",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("Save before quitting?"),
        Line::from(vec![
            quit_action_span(QuitAction::Save, app.quit_selected),
            Span::raw("  "),
            quit_action_span(QuitAction::Discard, app.quit_selected),
            Span::raw("  "),
            quit_action_span(QuitAction::Cancel, app.quit_selected),
        ]),
    ];
    if let Some(message) = &app.message {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            message.clone(),
            Style::default().fg(Color::Red),
        )));
    }

    let widget = Paragraph::new(lines).block(
        Block::default()
            .title(" QUIT ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(widget, popup);
}

fn quit_action_span(action: QuitAction, selected: QuitAction) -> Span<'static> {
    let (label, base_style) = match action {
        QuitAction::Save => (
            "[S] Save",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        QuitAction::Discard => (
            "[D] Discard",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        QuitAction::Cancel => ("[Esc] Cancel", Style::default().fg(Color::DarkGray)),
    };

    if action == selected {
        Span::styled(
            format!(" {} ", label),
            base_style
                .bg(Color::DarkGray)
                .add_modifier(Modifier::REVERSED),
        )
    } else {
        Span::styled(label.to_string(), base_style)
    }
}

fn centered_rect(area: ratatui::layout::Rect, width: u16, height: u16) -> ratatui::layout::Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(height.min(area.height)),
            Constraint::Fill(1),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(width.min(area.width)),
            Constraint::Fill(1),
        ])
        .split(vertical[1])[1]
}

fn split_line_for_display(text: &str, width: usize) -> Vec<String> {
    if width == 0 || text.is_empty() {
        return vec![text.to_string()];
    }
    let mut rows: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;
    for ch in text.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width + cw > width && !current.is_empty() {
            rows.push(current.clone());
            current.clear();
            current_width = 0;
        }
        current.push(ch);
        current_width += cw;
    }
    rows.push(current);
    rows
}

fn render_preview_row(app: &App, row: &PreviewRow) -> Line<'static> {
    match row.kind {
        PreviewRowKind::FenceStart => Line::from(Span::styled(
            " \u{2500}\u{2500}\u{2500} code \u{2500}\u{2500}\u{2500}".to_string(),
            Style::default().fg(Color::DarkGray),
        )),
        PreviewRowKind::FenceEnd => Line::from(Span::styled(
            " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}"
                .to_string(),
            Style::default().fg(Color::DarkGray),
        )),
        PreviewRowKind::Code => {
            let raw_line = app.buffer.line_text(row.source_line.expect("source line"));
            Line::from(Span::styled(
                format!("  {}", raw_line),
                Style::default().fg(Color::White).bg(Color::Rgb(30, 30, 30)),
            ))
        }
        PreviewRowKind::Heading(level) => {
            let raw_line = app.buffer.line_text(row.source_line.expect("source line"));
            let text = raw_line.trim().trim_start_matches('#').trim().to_string();
            let (color, prefix) = match level {
                1 => (Color::Magenta, "\u{2588} "),
                2 => (Color::Cyan, "\u{2590} "),
                3 => (Color::Blue, "  \u{25b8} "),
                _ => (Color::DarkGray, "    "),
            };
            Line::from(Span::styled(
                format!("{}{}", prefix, text),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ))
        }
        PreviewRowKind::Blockquote => {
            let raw_line = app.buffer.line_text(row.source_line.expect("source line"));
            let text = raw_line.trim().trim_start_matches('>').trim().to_string();
            Line::from(Span::styled(
                format!("  \u{2502} {}", text),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ))
        }
        PreviewRowKind::HorizontalRule => Line::from(Span::styled(
            "\u{2500}".repeat(40),
            Style::default().fg(Color::DarkGray),
        )),
        PreviewRowKind::List => {
            let raw_line = app.buffer.line_text(row.source_line.expect("source line"));
            let trimmed = raw_line.trim_start();
            let text = &trimmed[2..];
            let indent = raw_line.len() - trimmed.len();
            let pad = " ".repeat(indent);
            Line::from(vec![
                Span::raw(format!("{}  ", pad)),
                Span::styled(
                    "\u{2022} ".to_string(),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(text.to_string()),
            ])
        }
        PreviewRowKind::Plain => {
            let raw_line = app.buffer.line_text(row.source_line.expect("source line"));
            Line::from(format!("  {}", raw_line))
        }
        PreviewRowKind::Blank => Line::from(""),
    }
}
