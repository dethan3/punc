use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

pub fn highlight_line(text: &str) -> Line<'static> {
    let trimmed = text.trim_start();

    // Heading: # ## ### etc.
    if trimmed.starts_with('#') {
        let level = trimmed.chars().take_while(|&c| c == '#').count();
        let color = match level {
            1 => Color::Magenta,
            2 => Color::Cyan,
            3 => Color::Blue,
            _ => Color::DarkGray,
        };
        return Line::from(Span::styled(
            text.to_string(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ));
    }

    // Code block fence: ``` or ~~~
    if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
        return Line::from(Span::styled(
            text.to_string(),
            Style::default().fg(Color::DarkGray),
        ));
    }

    // Blockquote: > text
    if trimmed.starts_with('>') {
        return Line::from(Span::styled(
            text.to_string(),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        ));
    }

    // Unordered list: - or * or + item
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
        let indent_len = text.len() - trimmed.len();
        let indent = &text[..indent_len];
        let bullet = &trimmed[..2];
        let rest = &trimmed[2..];
        return Line::from(vec![
            Span::raw(indent.to_string()),
            Span::styled(
                bullet.to_string(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(rest.to_string()),
        ]);
    }

    // Ordered list: 1. 2. etc.
    if let Some(pos) = trimmed.find(". ") {
        let prefix = &trimmed[..pos];
        if prefix.chars().all(|c| c.is_ascii_digit()) {
            let indent_len = text.len() - trimmed.len();
            let indent = &text[..indent_len];
            let num_dot = &trimmed[..pos + 2];
            let rest = &trimmed[pos + 2..];
            return Line::from(vec![
                Span::raw(indent.to_string()),
                Span::styled(
                    num_dot.to_string(),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(rest.to_string()),
            ]);
        }
    }

    // Horizontal rule: --- or *** or ___
    if trimmed == "---" || trimmed == "***" || trimmed == "___" {
        return Line::from(Span::styled(
            text.to_string(),
            Style::default().fg(Color::DarkGray),
        ));
    }

    // Regular text — do inline highlighting
    inline_highlight(text)
}

fn inline_highlight(text: &str) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut i = 0;
    let mut plain_start = 0;

    while i < text.len() {
        let rest = &text[i..];

        // Bold: **text** or __text__
        if rest.starts_with("**") || rest.starts_with("__") {
            let delim = &rest[..2];
            if let Some(end) = rest[2..].find(delim) {
                if plain_start < i {
                    spans.push(Span::raw(text[plain_start..i].to_string()));
                }
                spans.push(Span::styled(
                    rest[..end + 4].to_string(),
                    Style::default().add_modifier(Modifier::BOLD),
                ));
                i += end + 4;
                plain_start = i;
                continue;
            }
        }

        // Inline code: `text`
        if rest.starts_with('`') {
            if let Some(end) = rest[1..].find('`') {
                if plain_start < i {
                    spans.push(Span::raw(text[plain_start..i].to_string()));
                }
                spans.push(Span::styled(
                    rest[..end + 2].to_string(),
                    Style::default().fg(Color::Red).bg(Color::Rgb(40, 40, 40)),
                ));
                i += end + 2;
                plain_start = i;
                continue;
            }
        }

        i += rest.chars().next().expect("slice is non-empty").len_utf8();
    }

    if plain_start < text.len() {
        spans.push(Span::raw(text[plain_start..].to_string()));
    }

    if spans.is_empty() {
        Line::from(String::new())
    } else {
        Line::from(spans)
    }
}
