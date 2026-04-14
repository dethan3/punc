use similar::{ChangeTag, TextDiff};

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub tag: DiffTag,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DiffTag {
    Equal,
    Insert,
    Delete,
}

pub fn compute_diff(old: &str, new: &str) -> Vec<DiffLine> {
    let diff = TextDiff::from_lines(old, new);
    let mut lines = Vec::new();

    for change in diff.iter_all_changes() {
        let tag = match change.tag() {
            ChangeTag::Equal => DiffTag::Equal,
            ChangeTag::Insert => DiffTag::Insert,
            ChangeTag::Delete => DiffTag::Delete,
        };
        let mut text = change.to_string();
        if text.ends_with('\n') {
            text.pop();
        }
        lines.push(DiffLine { tag, text });
    }

    lines
}

pub fn has_changes(diff_lines: &[DiffLine]) -> bool {
    diff_lines.iter().any(|l| l.tag != DiffTag::Equal)
}
