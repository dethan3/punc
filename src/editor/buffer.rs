use ropey::Rope;
use std::fs;
use std::path::{Path, PathBuf};

use super::Cursor;
use super::undo::{Snapshot, UndoStack};

pub struct Buffer {
    pub rope: Rope,
    pub cursor: Cursor,
    pub scroll_offset: usize,
    pub dirty: bool,
    pub path: PathBuf,
    pub undo_stack: UndoStack,
}

impl Buffer {
    pub fn from_file(path: &Path) -> std::io::Result<Self> {
        let content = if path.exists() {
            fs::read_to_string(path)?
        } else {
            String::new()
        };
        let rope = Rope::from_str(&content);
        Ok(Self {
            rope,
            cursor: Cursor::new(),
            scroll_offset: 0,
            dirty: false,
            path: path.to_path_buf(),
            undo_stack: UndoStack::new(),
        })
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        let content = self.rope.to_string();
        fs::write(&self.path, &content)?;
        self.dirty = false;
        Ok(())
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
            self.dirty = true;
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
            self.dirty = true;
        }
    }

    pub fn insert_char(&mut self, ch: char) {
        self.save_snapshot();
        let idx = self.cursor.char_index(&self.rope);
        self.rope.insert_char(idx, ch);
        if ch == '\n' {
            self.cursor.line += 1;
            self.cursor.col = 0;
        } else {
            self.cursor.col += 1;
        }
        self.dirty = true;
    }

    pub fn backspace(&mut self) {
        let idx = self.cursor.char_index(&self.rope);
        if idx == 0 {
            return;
        }
        self.save_snapshot();
        self.cursor.move_left(&self.rope);
        self.rope.remove(idx - 1..idx);
        self.dirty = true;
    }

    pub fn delete(&mut self) {
        let idx = self.cursor.char_index(&self.rope);
        if idx >= self.rope.len_chars() {
            return;
        }
        self.save_snapshot();
        self.rope.remove(idx..idx + 1);
        self.dirty = true;
    }

    pub fn paste(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.save_snapshot();
        let idx = self.cursor.char_index(&self.rope);
        self.rope.insert(idx, text);
        // Move cursor to end of pasted text
        for ch in text.chars() {
            if ch == '\n' {
                self.cursor.line += 1;
                self.cursor.col = 0;
            } else {
                self.cursor.col += 1;
            }
        }
        self.dirty = true;
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
        (start..end).map(|i| {
            let line = self.rope.line(i);
            let mut s = line.to_string();
            if s.ends_with('\n') {
                s.pop();
            }
            (i, s)
        })
    }

    pub fn adjust_scroll(&mut self, height: usize) {
        if self.cursor.line < self.scroll_offset {
            self.scroll_offset = self.cursor.line;
        }
        if self.cursor.line >= self.scroll_offset + height {
            self.scroll_offset = self.cursor.line - height + 1;
        }
    }

    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn current_section(&self) -> Option<String> {
        for i in (0..=self.cursor.line).rev() {
            let line = self.rope.line(i).to_string();
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') {
                let heading = trimmed.trim_start_matches('#').trim();
                return Some(heading.to_string());
            }
        }
        None
    }
}
