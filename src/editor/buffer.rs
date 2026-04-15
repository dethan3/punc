use ropey::{Rope, RopeSlice};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use super::undo::{Snapshot, UndoStack};
use super::Cursor;

pub struct Buffer {
    pub rope: Rope,
    pub cursor: Cursor,
    pub scroll_offset: usize,
    pub dirty: bool,
    pub path: PathBuf,
    pub display_name: String,
    revision: u64,
    pub undo_stack: UndoStack,
}

impl Buffer {
    pub fn from_file(path: &Path) -> std::io::Result<Self> {
        let resolved_path = if path.exists() {
            path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
        } else {
            path.to_path_buf()
        };
        let rope = if resolved_path.exists() {
            Rope::from_reader(File::open(&resolved_path)?)?
        } else {
            Rope::new()
        };
        let display_name = resolved_path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".to_string());
        Ok(Self {
            rope,
            cursor: Cursor::new(),
            scroll_offset: 0,
            dirty: false,
            path: resolved_path,
            display_name,
            revision: 0,
            undo_stack: UndoStack::new(),
        })
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        let file = File::create(&self.path)?;
        let mut writer = BufWriter::new(file);
        self.rope.write_to(&mut writer)?;
        writer.flush()?;
        self.dirty = false;
        Ok(())
    }

    pub fn content_revision(&self) -> u64 {
        self.revision
    }

    pub fn replace_content(&mut self, rope: Rope) {
        self.rope = rope;
        self.mark_modified();
    }

    pub fn save_snapshot(&mut self) {
        let snap = Snapshot {
            content: self.rope.clone(),
            cursor_line: self.cursor.line,
            cursor_col: self.cursor.col,
        };
        self.undo_stack.push(snap);
    }

    pub fn undo(&mut self) {
        let current = Snapshot {
            content: self.rope.clone(),
            cursor_line: self.cursor.line,
            cursor_col: self.cursor.col,
        };
        if let Some(snap) = self.undo_stack.undo(current) {
            self.rope = snap.content;
            self.cursor.line = snap.cursor_line;
            self.cursor.col = snap.cursor_col;
            self.mark_modified();
        }
    }

    pub fn redo(&mut self) {
        let current = Snapshot {
            content: self.rope.clone(),
            cursor_line: self.cursor.line,
            cursor_col: self.cursor.col,
        };
        if let Some(snap) = self.undo_stack.redo(current) {
            self.rope = snap.content;
            self.cursor.line = snap.cursor_line;
            self.cursor.col = snap.cursor_col;
            self.mark_modified();
        }
    }

    pub fn insert_char(&mut self, ch: char) {
        let mut buf = [0; 4];
        self.insert_text(ch.encode_utf8(&mut buf));
    }

    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        self.save_snapshot();
        let idx = self.cursor.char_index(&self.rope);
        self.rope.insert(idx, text);
        self.advance_cursor(text);
        self.mark_modified();
    }

    pub fn backspace(&mut self) {
        let idx = self.cursor.char_index(&self.rope);
        if idx == 0 {
            return;
        }
        self.save_snapshot();
        self.cursor.move_left(&self.rope);
        self.rope.remove(idx - 1..idx);
        self.mark_modified();
    }

    pub fn delete(&mut self) {
        let idx = self.cursor.char_index(&self.rope);
        if idx >= self.rope.len_chars() {
            return;
        }
        self.save_snapshot();
        self.rope.remove(idx..idx + 1);
        self.mark_modified();
    }

    pub fn paste(&mut self, text: &str) {
        self.insert_text(text);
    }

    pub fn line_text(&self, line_idx: usize) -> String {
        let line = self.rope.line(line_idx);
        let mut text = line.to_string();
        if text.ends_with('\n') {
            text.pop();
        }
        text
    }

    pub fn page_up(&mut self, height: usize) {
        let jump = height.saturating_sub(2);
        self.cursor.line = self.cursor.line.saturating_sub(jump);
        self.cursor.clamp_col(&self.rope);
        self.scroll_offset = self.scroll_offset.saturating_sub(jump);
    }

    pub fn page_down(&mut self, height: usize) {
        let jump = height.saturating_sub(2);
        let max_line = self.rope.len_lines().saturating_sub(1);
        self.cursor.line = (self.cursor.line + jump).min(max_line);
        self.cursor.clamp_col(&self.rope);
        self.scroll_offset = (self.scroll_offset + jump).min(max_line);
    }

    pub fn visible_lines(&self, height: usize) -> impl Iterator<Item = (usize, String)> + '_ {
        let total = self.rope.len_lines();
        let start = self.scroll_offset;
        let end = (start + height).min(total);
        (start..end).map(|i| (i, self.line_text(i)))
    }

    pub fn adjust_scroll(&mut self, height: usize) {
        if self.cursor.line < self.scroll_offset {
            self.scroll_offset = self.cursor.line;
        }
        if self.cursor.line >= self.scroll_offset + height {
            self.scroll_offset = self.cursor.line - height + 1;
        }
    }

    pub fn current_section(&self) -> Option<String> {
        let last_line = self.rope.len_lines().saturating_sub(1);
        for i in (0..=self.cursor.line.min(last_line)).rev() {
            if let Some((_, heading)) = parse_heading(self.rope.line(i)) {
                return Some(heading);
            }
        }
        None
    }

    pub fn heading_at(&self, line_idx: usize) -> Option<(usize, String)> {
        parse_heading(self.rope.line(line_idx))
    }

    fn mark_modified(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.dirty = true;
    }

    fn advance_cursor(&mut self, text: &str) {
        for ch in text.chars() {
            if ch == '\n' {
                self.cursor.line += 1;
                self.cursor.col = 0;
            } else {
                self.cursor.col += 1;
            }
        }
    }
}

fn parse_heading(line: RopeSlice<'_>) -> Option<(usize, String)> {
    let mut chars = line.chars().peekable();

    while matches!(chars.peek(), Some(' ' | '\t')) {
        chars.next();
    }

    let mut level = 0;
    while matches!(chars.peek(), Some('#')) {
        chars.next();
        level += 1;
    }

    if level == 0 {
        return None;
    }

    while matches!(chars.peek(), Some(' ' | '\t')) {
        chars.next();
    }

    let mut text = String::new();
    for ch in chars {
        if ch != '\n' {
            text.push(ch);
        }
    }

    Some((level, text.trim().to_string()))
}
