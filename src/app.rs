use std::path::Path;

use crate::diff::{DiffLine, compute_diff};
use crate::editor::Buffer;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Edit,
    Preview,
    Outline,
    Diff,
}

#[derive(Debug, Clone)]
pub struct HeadingEntry {
    pub level: usize,
    pub text: String,
    pub line: usize,
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
        })
    }

    pub fn status_line(&self) -> String {
        let file_name = self
            .buffer
            .path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".to_string());
        let dirty = if self.buffer.dirty { " *" } else { "" };
        let ext = if self.external_change { " \u{26a1}" } else { "" };
        let mode = match self.mode {
            Mode::Edit => "EDIT",
            Mode::Preview => "PREVIEW",
            Mode::Outline => "OUTLINE",
            Mode::Diff => "DIFF",
        };
        let ln = self.buffer.cursor.line + 1;
        let section = self
            .buffer
            .current_section()
            .map(|s| format!(" | Section: {}", s))
            .unwrap_or_default();

        format!("{}{}{} | {} | Ln {}{}", file_name, dirty, ext, mode, ln, section)
    }

    pub fn build_outline(&mut self) {
        self.outline_entries.clear();
        for i in 0..self.buffer.rope.len_lines() {
            let line = self.buffer.rope.line(i).to_string();
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                let level = trimmed.chars().take_while(|&c| c == '#').count();
                let text = trimmed.trim_start_matches('#').trim().to_string();
                self.outline_entries.push(HeadingEntry { level, text, line: i });
            }
        }
        self.outline_selected = 0;
    }

    pub fn handle_external_change(&mut self, new_content: String) {
        let current = self.buffer.rope.to_string();
        if new_content != current {
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
            self.buffer.rope = ropey::Rope::from_str(&content);
            self.buffer.cursor.clamp_col(&self.buffer.rope);
            self.buffer.dirty = true;
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
}
