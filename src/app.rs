use std::path::Path;

use ropey::RopeSlice;

use crate::diff::{compute_diff, DiffLine};
use crate::editor::Buffer;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Edit,
    Preview,
    Outline,
    Diff,
    QuitConfirm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitAction {
    Save,
    Discard,
    Cancel,
}

#[derive(Debug, Clone)]
pub struct HeadingEntry {
    pub level: usize,
    pub text: String,
    pub line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewRowKind {
    FenceStart,
    FenceEnd,
    Code,
    Heading(usize),
    Blockquote,
    HorizontalRule,
    List,
    Plain,
    Blank,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewRow {
    pub source_line: Option<usize>,
    pub kind: PreviewRowKind,
}

#[derive(Debug, Clone)]
struct PreviewCache {
    revision: u64,
    rows: Vec<PreviewRow>,
}

#[derive(Debug, Clone)]
struct SectionCache {
    revision: u64,
    cursor_line: usize,
    value: Option<String>,
}

pub struct App {
    pub buffer: Buffer,
    pub mode: Mode,
    pub should_quit: bool,
    pub message: Option<String>,
    pub preview_scroll: usize,
    pub outline_entries: Vec<HeadingEntry>,
    pub outline_selected: usize,
    pub external_change: bool,
    pub external_content: Option<String>,
    pub diff_lines: Vec<DiffLine>,
    pub diff_scroll: usize,
    pub quit_return_mode: Mode,
    pub quit_selected: QuitAction,
    preview_cache: Option<PreviewCache>,
    section_cache: Option<SectionCache>,
}

impl App {
    pub fn new(path: &Path) -> std::io::Result<Self> {
        let buffer = Buffer::from_file(path)?;
        Ok(Self {
            buffer,
            mode: Mode::Edit,
            should_quit: false,
            message: None,
            preview_scroll: 0,
            outline_entries: Vec::new(),
            outline_selected: 0,
            external_change: false,
            external_content: None,
            diff_lines: Vec::new(),
            diff_scroll: 0,
            quit_return_mode: Mode::Edit,
            quit_selected: QuitAction::Save,
            preview_cache: None,
            section_cache: None,
        })
    }

    pub fn status_line(&mut self) -> String {
        let file_name = self.buffer.display_name.clone();
        let dirty = if self.buffer.dirty { " *" } else { "" };
        let ext = if self.external_change {
            " \u{26a1}"
        } else {
            ""
        };
        let mode = match self.mode {
            Mode::Edit => "EDIT",
            Mode::Preview => "PREVIEW",
            Mode::Outline => "OUTLINE",
            Mode::Diff => "DIFF",
            Mode::QuitConfirm => "QUIT",
        };
        let ln = self
            .buffer
            .cursor
            .line
            .min(self.buffer.rope.len_lines().saturating_sub(1))
            + 1;
        let section = self
            .current_section_cached()
            .map(|s| format!(" | Section: {}", s))
            .unwrap_or_default();

        format!(
            "{}{}{} | {} | Ln {}{}",
            file_name, dirty, ext, mode, ln, section
        )
    }

    pub fn build_outline(&mut self) {
        self.outline_entries.clear();
        for i in 0..self.buffer.rope.len_lines() {
            if let Some((level, text)) = self.buffer.heading_at(i) {
                self.outline_entries.push(HeadingEntry {
                    level,
                    text,
                    line: i,
                });
            }
        }
        self.outline_selected = 0;
    }

    pub fn handle_external_change(&mut self, new_content: String) {
        if self.buffer.rope == new_content.as_str() {
            self.external_change = false;
            self.external_content = None;
            self.diff_lines.clear();
            if self.mode == Mode::Diff {
                self.mode = Mode::Edit;
            }
        } else {
            self.external_change = true;
            self.external_content = Some(new_content);
        }
    }

    pub fn open_diff(&mut self) {
        if let Some(ref ext) = self.external_content {
            let current = self.buffer.rope.to_string();
            self.diff_lines = compute_diff(&current, ext);
            self.diff_scroll = 0;
            self.mode = Mode::Diff;
        }
    }

    pub fn accept_external(&mut self) {
        if let Some(content) = self.external_content.take() {
            self.buffer.save_snapshot();
            self.buffer
                .replace_content_from_disk(ropey::Rope::from_str(&content));
            self.buffer.cursor.clamp(&self.buffer.rope);
            self.buffer.scroll_offset = self
                .buffer
                .scroll_offset
                .min(self.buffer.rope.len_lines().saturating_sub(1));
        }
        self.external_change = false;
        self.diff_lines.clear();
        self.mode = Mode::Edit;
    }

    pub fn reject_external(&mut self) {
        self.external_content = None;
        self.external_change = false;
        self.diff_lines.clear();
        self.mode = Mode::Edit;
    }

    pub fn request_quit(&mut self) {
        if self.buffer.dirty {
            self.quit_return_mode = self.mode;
            self.quit_selected = QuitAction::Save;
            self.mode = Mode::QuitConfirm;
        } else {
            self.should_quit = true;
        }
    }

    pub fn cancel_quit(&mut self) {
        self.mode = self.quit_return_mode;
    }

    pub fn discard_and_quit(&mut self) {
        self.should_quit = true;
    }

    pub fn save_and_quit(&mut self) {
        match self.buffer.save() {
            Ok(()) => {
                self.message = Some("Saved".to_string());
                self.should_quit = true;
            }
            Err(e) => {
                self.message = Some(format!("Save failed: {}", e));
            }
        }
    }

    pub fn select_next_quit_action(&mut self) {
        self.quit_selected = match self.quit_selected {
            QuitAction::Save => QuitAction::Discard,
            QuitAction::Discard => QuitAction::Cancel,
            QuitAction::Cancel => QuitAction::Save,
        };
    }

    pub fn select_prev_quit_action(&mut self) {
        self.quit_selected = match self.quit_selected {
            QuitAction::Save => QuitAction::Cancel,
            QuitAction::Discard => QuitAction::Save,
            QuitAction::Cancel => QuitAction::Discard,
        };
    }

    pub fn preview_row_count(&mut self) -> usize {
        self.preview_cache().rows.len()
    }

    pub fn preview_visible_rows(&mut self, offset: usize, height: usize) -> Vec<PreviewRow> {
        let rows = &self.preview_cache().rows;
        let start = offset.min(rows.len().saturating_sub(1));
        rows.iter().skip(start).take(height).copied().collect()
    }

    fn current_section_cached(&mut self) -> Option<&str> {
        let revision = self.buffer.content_revision();
        let cursor_line = self
            .buffer
            .cursor
            .line
            .min(self.buffer.rope.len_lines().saturating_sub(1));
        let cache_is_fresh = self
            .section_cache
            .as_ref()
            .is_some_and(|cache| cache.revision == revision && cache.cursor_line == cursor_line);

        if !cache_is_fresh {
            self.section_cache = Some(SectionCache {
                revision,
                cursor_line,
                value: self.buffer.current_section(),
            });
        }

        self.section_cache
            .as_ref()
            .and_then(|cache| cache.value.as_deref())
    }

    fn preview_cache(&mut self) -> &PreviewCache {
        let revision = self.buffer.content_revision();
        let cache_is_fresh = self
            .preview_cache
            .as_ref()
            .is_some_and(|cache| cache.revision == revision);

        if !cache_is_fresh {
            self.preview_cache = Some(self.build_preview_cache());
        }

        self.preview_cache
            .as_ref()
            .expect("preview cache initialized")
    }

    fn build_preview_cache(&self) -> PreviewCache {
        let mut rows = Vec::with_capacity(self.buffer.rope.len_lines());
        let mut in_code_block = false;

        for line_idx in 0..self.buffer.rope.len_lines() {
            let line = self.buffer.rope.line(line_idx);

            if is_code_fence(line) {
                in_code_block = !in_code_block;
                rows.push(PreviewRow {
                    source_line: None,
                    kind: if in_code_block {
                        PreviewRowKind::FenceStart
                    } else {
                        PreviewRowKind::FenceEnd
                    },
                });
                continue;
            }

            if in_code_block {
                rows.push(PreviewRow {
                    source_line: Some(line_idx),
                    kind: PreviewRowKind::Code,
                });
                continue;
            }

            if let Some(level) = heading_level(line) {
                rows.push(PreviewRow {
                    source_line: Some(line_idx),
                    kind: PreviewRowKind::Heading(level),
                });
                if level <= 2 {
                    rows.push(PreviewRow {
                        source_line: None,
                        kind: PreviewRowKind::Blank,
                    });
                }
                continue;
            }

            let kind = if is_blockquote(line) {
                PreviewRowKind::Blockquote
            } else if is_horizontal_rule(line) {
                PreviewRowKind::HorizontalRule
            } else if is_list_item(line) {
                PreviewRowKind::List
            } else if is_blank(line) {
                PreviewRowKind::Blank
            } else {
                PreviewRowKind::Plain
            };

            rows.push(PreviewRow {
                source_line: Some(line_idx),
                kind,
            });
        }

        PreviewCache {
            revision: self.buffer.content_revision(),
            rows,
        }
    }
}

fn is_blank(line: RopeSlice<'_>) -> bool {
    trimmed_chars(line).next().is_none()
}

fn is_code_fence(line: RopeSlice<'_>) -> bool {
    let mut chars = trimmed_chars(line);
    match chars.next() {
        Some('`') => chars.next() == Some('`') && chars.next() == Some('`'),
        Some('~') => chars.next() == Some('~') && chars.next() == Some('~'),
        _ => false,
    }
}

fn heading_level(line: RopeSlice<'_>) -> Option<usize> {
    let mut chars = trimmed_chars(line).peekable();
    let mut level = 0;
    while matches!(chars.peek(), Some('#')) {
        chars.next();
        level += 1;
    }
    (level > 0).then_some(level)
}

fn is_blockquote(line: RopeSlice<'_>) -> bool {
    trimmed_chars(line).next() == Some('>')
}

fn is_list_item(line: RopeSlice<'_>) -> bool {
    let mut chars = trimmed_chars(line);
    match chars.next() {
        Some('-' | '*' | '+') => chars.next() == Some(' '),
        _ => false,
    }
}

fn is_horizontal_rule(line: RopeSlice<'_>) -> bool {
    let mut chars = trimmed_chars(line);
    let Some(first) = chars.next() else {
        return false;
    };
    if !matches!(first, '-' | '*' | '_') {
        return false;
    }
    chars.next() == Some(first) && chars.next() == Some(first) && chars.next().is_none()
}

fn trimmed_chars(line: RopeSlice<'_>) -> impl Iterator<Item = char> + '_ {
    let mut seen_content = false;
    line.chars().filter(move |&ch| {
        if ch == '\n' {
            return false;
        }
        if !seen_content && matches!(ch, ' ' | '\t') {
            return false;
        }
        seen_content = true;
        true
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file_path(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("punc-{name}-{unique}.md"))
    }

    #[test]
    fn accept_external_clamps_cursor_after_shorter_file() {
        let path = temp_file_path("accept-external");
        fs::write(&path, "# Heading\nline 2\nline 3\nline 4\n").unwrap();

        let mut app = App::new(&path).unwrap();
        app.buffer.cursor.line = 3;
        app.buffer.cursor.col = 99;
        app.handle_external_change("# New heading".to_string());

        app.accept_external();

        assert_eq!(app.buffer.cursor.line, 0);
        assert_eq!(app.buffer.cursor.col, "# New heading".chars().count());
        let status = app.status_line();
        assert!(status.contains("Ln 1"));
        assert!(status.contains("Section: New heading"));

        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn accept_external_leaves_buffer_clean() {
        let path = temp_file_path("accept-external-clean");
        fs::write(&path, "hello\n").unwrap();

        let mut app = App::new(&path).unwrap();
        app.handle_external_change("hello from outside\n".to_string());

        app.accept_external();

        assert!(!app.buffer.dirty);
        app.request_quit();
        assert!(app.should_quit);
        assert_ne!(app.mode, Mode::QuitConfirm);

        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn external_change_clears_when_disk_matches_buffer_again() {
        let path = temp_file_path("external-clear");
        fs::write(&path, "hello\n").unwrap();

        let mut app = App::new(&path).unwrap();
        app.handle_external_change("hello from outside\n".to_string());
        app.mode = Mode::Diff;
        app.diff_lines = vec![DiffLine {
            tag: crate::diff::DiffTag::Insert,
            text: "hello from outside".to_string(),
        }];

        app.handle_external_change("hello\n".to_string());

        assert!(!app.external_change);
        assert!(app.external_content.is_none());
        assert!(app.diff_lines.is_empty());
        assert_eq!(app.mode, Mode::Edit);

        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn request_quit_with_dirty_buffer_enters_quit_confirm() {
        let path = temp_file_path("quit-confirm");
        fs::write(&path, "hello\n").unwrap();

        let mut app = App::new(&path).unwrap();
        app.buffer.insert_char('!');
        app.mode = Mode::Preview;

        app.request_quit();

        assert_eq!(app.mode, Mode::QuitConfirm);
        assert_eq!(app.quit_return_mode, Mode::Preview);
        assert_eq!(app.quit_selected, QuitAction::Save);
        assert!(!app.should_quit);

        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn save_and_quit_persists_changes() {
        let path = temp_file_path("save-and-quit");
        fs::write(&path, "hello\n").unwrap();

        let mut app = App::new(&path).unwrap();
        app.buffer.insert_char('!');

        app.request_quit();
        app.save_and_quit();

        assert!(app.should_quit);
        assert!(!app.buffer.dirty);
        assert_eq!(fs::read_to_string(&path).unwrap(), "!hello\n");

        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn undo_and_redo_track_saved_state_cleanly() {
        let path = temp_file_path("undo-clean-state");
        fs::write(&path, "hello\n").unwrap();

        let mut app = App::new(&path).unwrap();
        app.buffer.insert_char('!');
        assert!(app.buffer.dirty);

        app.buffer.undo();
        assert!(!app.buffer.dirty);
        assert_eq!(app.buffer.rope.to_string(), "hello\n");

        app.buffer.redo();
        assert!(app.buffer.dirty);
        assert_eq!(app.buffer.rope.to_string(), "!hello\n");

        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn paste_with_crlf_updates_cursor_using_rope_coordinates() {
        let path = temp_file_path("paste-crlf");
        fs::write(&path, "").unwrap();

        let mut app = App::new(&path).unwrap();
        app.buffer.insert_text("a\r\nb");

        assert_eq!(app.buffer.cursor.line, 1);
        assert_eq!(app.buffer.cursor.col, 1);
        assert_eq!(app.buffer.rope.to_string(), "a\r\nb");

        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn quit_confirm_selection_wraps() {
        let path = temp_file_path("quit-selection");
        fs::write(&path, "hello\n").unwrap();

        let mut app = App::new(&path).unwrap();
        app.buffer.insert_char('!');
        app.request_quit();

        app.select_next_quit_action();
        assert_eq!(app.quit_selected, QuitAction::Discard);
        app.select_next_quit_action();
        assert_eq!(app.quit_selected, QuitAction::Cancel);
        app.select_next_quit_action();
        assert_eq!(app.quit_selected, QuitAction::Save);

        app.select_prev_quit_action();
        assert_eq!(app.quit_selected, QuitAction::Cancel);

        fs::remove_file(&path).unwrap();
    }
}
