use ropey::Rope;

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub content: Rope,
    pub cursor_line: usize,
    pub cursor_col: usize,
}

pub struct UndoStack {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    max_size: usize,
}

impl UndoStack {
    pub fn new() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            max_size: 1000,
        }
    }

    pub fn push(&mut self, snapshot: Snapshot) {
        if self.undo.len() >= self.max_size {
            self.undo.remove(0);
        }
        self.undo.push(snapshot);
        self.redo.clear();
    }

    pub fn undo(&mut self, current: Snapshot) -> Option<Snapshot> {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(current);
            Some(prev)
        } else {
            None
        }
    }

    pub fn redo(&mut self, current: Snapshot) -> Option<Snapshot> {
        if let Some(next) = self.redo.pop() {
            self.undo.push(current);
            Some(next)
        } else {
            None
        }
    }
}
