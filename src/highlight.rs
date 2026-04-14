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
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
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
            Span::styled(bullet.to_string(), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
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
                Span::styled(num_dot.to_string(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
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
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut buf = String::new();

    while i < len {
        // Bold: **text** or __text__
        if i + 1 < len && ((chars[i] == '*' && chars[i + 1] == '*') || (chars[i] == '_' && chars[i + 1] == '_')) {
            let delim = chars[i];
            if let Some(end) = find_closing_double(&chars, i + 2, delim) {
                if !buf.is_empty() {
                    spans.push(Span::raw(buf.clone()));
                    buf.clear();
                }
                let content: String = chars[i + 2..end].iter().collect();
                spans.push(Span::styled(
                    format!("{0}{0}{1}{0}{0}", delim, content),
                    Style::default().add_modifier(Modifier::BOLD),
                ));
                i = end + 2;
                continue;
            }
        }

        // Inline code: `text`
        if chars[i] == '`' {
            if let Some(end) = find_closing_single(&chars, i + 1, '`') {
                if !buf.is_empty() {
                    spans.push(Span::raw(buf.clone()));
                    buf.clear();
                }
                let content: String = chars[i..=end].iter().collect();
                spans.push(Span::styled(
                    content,
                    Style::default().fg(Color::Red).bg(Color::Rgb(40, 40, 40)),
                ));
                i = end + 1;
                continue;
            }
        }

        buf.push(chars[i]);
        i += 1;
    }

    if !buf.is_empty() {
        spans.push(Span::raw(buf));
    }

    if spans.is_empty() {
        Line::from(String::new())
    } else {
        Line::from(spans)
    }
}

fn find_closing_double(chars: &[char], start: usize, delim: char) -> Option<usize> {
    let len = chars.len();
    let mut i = start;
    while i + 1 < len {
        if chars[i] == delim && chars[i + 1] == delim {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_closing_single(chars: &[char], start: usize, delim: char) -> Option<usize> {
    for i in start..chars.len() {
        if chars[i] == delim {
            return Some(i);
        }
    }
    None
}
