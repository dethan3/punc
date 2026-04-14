use ropey::Rope;

#[derive(Debug, Clone)]
pub struct Cursor {
    pub line: usize,
    pub col: usize,
}

impl Cursor {
    pub fn new() -> Self {
        Self { line: 0, col: 0 }
    }

    pub fn clamp(&mut self, rope: &Rope) {
        self.line = self.line.min(last_line_index(rope));
        self.clamp_col(rope);
    }

    pub fn move_up(&mut self, rope: &Rope) {
        if self.line > 0 {
            self.line -= 1;
            self.clamp_col(rope);
        }
    }

    pub fn move_down(&mut self, rope: &Rope) {
        if self.line + 1 < rope.len_lines() {
            self.line += 1;
            self.clamp_col(rope);
        }
    }

    pub fn move_left(&mut self, rope: &Rope) {
        if self.col > 0 {
            self.col -= 1;
        } else if self.line > 0 {
            self.line -= 1;
            self.col = line_len(rope, self.line);
        }
    }

    pub fn move_right(&mut self, rope: &Rope) {
        let len = line_len(rope, self.line);
        if self.col < len {
            self.col += 1;
        } else if self.line + 1 < rope.len_lines() {
            self.line += 1;
            self.col = 0;
        }
    }

    pub fn move_home(&mut self) {
        self.col = 0;
    }

    pub fn move_end(&mut self, rope: &Rope) {
        self.col = line_len(rope, self.line);
    }

    pub fn clamp_col(&mut self, rope: &Rope) {
        let len = line_len(rope, self.line);
        if self.col > len {
            self.col = len;
        }
    }

    pub fn char_index(&self, rope: &Rope) -> usize {
        let line = self.line.min(last_line_index(rope));
        let line_start = rope.line_to_char(line);
        let len = line_len(rope, line);
        line_start + self.col.min(len)
    }
}

fn last_line_index(rope: &Rope) -> usize {
    rope.len_lines().saturating_sub(1)
}

fn line_len(rope: &Rope, line: usize) -> usize {
    if line >= rope.len_lines() {
        return 0;
    }
    let line_slice = rope.line(line);
    let len = line_slice.len_chars();
    if len > 0 && line_slice.char(len - 1) == '\n' {
        len - 1
    } else {
        len
    }
}
