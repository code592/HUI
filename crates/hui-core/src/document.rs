use ropey::Rope;
use std::ops::Range;
use thiserror::Error;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub head: usize,
}

#[derive(Clone, Debug)]
pub struct Edit {
    pub range: Range<usize>,
    pub insert: String,
}

#[derive(Clone)]
struct HistoryEntry {
    before: Rope,
    after: Rope,
    before_selection: Selection,
    after_selection: Selection,
}

#[derive(Debug, Error)]
pub enum EditError {
    #[error("文档版本已变化，请重试")]
    StaleRevision,
    #[error("编辑范围越界或相互重叠")]
    InvalidRange,
}

#[derive(Clone, Debug)]
pub struct Viewport {
    pub chars: Range<usize>,
    pub first_line: usize,
    pub text: String,
    pub revision: u64,
}

pub struct Document {
    rope: Rope,
    revision: u64,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    pub selection: Selection,
    saved: Rope,
    dirty: bool,
}

impl Document {
    pub fn new(text: &str) -> Self {
        Self::from_rope(Rope::from_str(text))
    }
    pub fn from_rope(rope: Rope) -> Self {
        Self {
            saved: rope.clone(),
            rope,
            revision: 0,
            undo: vec![],
            redo: vec![],
            selection: Selection::default(),
            dirty: false,
        }
    }
    pub fn snapshot(&self) -> Rope {
        self.rope.clone()
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }
    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }
    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    pub fn mark_saved(&mut self, snapshot: Rope) {
        self.dirty = self.rope != snapshot;
        self.saved = snapshot;
    }
    pub fn line_of(&self, position: usize) -> usize {
        self.rope.char_to_line(position.min(self.len_chars()))
    }
    pub fn line_start(&self, line: usize) -> usize {
        self.rope.line_to_char(line.min(self.len_lines() - 1))
    }
    pub fn newline(&self) -> &'static str {
        // Inspect only the first line; preserve all unchanged line endings verbatim.
        if self.rope.line(0).chars().any(|c| c == '\r') {
            "\r\n"
        } else {
            "\n"
        }
    }
    pub fn apply(&mut self, revision: u64, mut edits: Vec<Edit>) -> Result<(), EditError> {
        if revision != self.revision {
            return Err(EditError::StaleRevision);
        }
        edits.sort_by_key(|e| (e.range.start, e.range.end));
        let mut end = 0;
        for (i, e) in edits.iter().enumerate() {
            if e.range.start > e.range.end
                || e.range.end > self.len_chars()
                || (i > 0 && e.range.start < end)
            {
                return Err(EditError::InvalidRange);
            }
            end = e.range.end;
        }
        if edits.is_empty() {
            return Ok(());
        }
        let before = self.rope.clone();
        let before_selection = self.selection.clone();
        for e in edits.iter().rev() {
            self.rope.remove(e.range.clone());
            self.rope.insert(e.range.start, &e.insert);
        }
        let first = &edits[0];
        let head = first.range.start + first.insert.chars().count();
        self.selection = Selection { anchor: head, head };
        self.revision += 1;
        self.dirty = true;
        self.undo.push(HistoryEntry {
            before,
            after: self.rope.clone(),
            before_selection,
            after_selection: self.selection.clone(),
        });
        // Shared Rope snapshots retain modified paths only. Bound the history as well.
        if self.undo.len() > 512 {
            self.undo.remove(0);
        }
        self.redo.clear();
        Ok(())
    }
    pub fn undo(&mut self) -> bool {
        let Some(entry) = self.undo.pop() else {
            return false;
        };
        self.rope = entry.before.clone();
        self.selection = entry.before_selection.clone();
        self.dirty = self.rope != self.saved;
        self.redo.push(entry);
        self.revision += 1;
        true
    }
    pub fn redo(&mut self) -> bool {
        let Some(entry) = self.redo.pop() else {
            return false;
        };
        self.rope = entry.after.clone();
        self.selection = entry.after_selection.clone();
        self.dirty = self.rope != self.saved;
        self.undo.push(entry);
        self.revision += 1;
        true
    }
    pub fn viewport(&self, start: usize, max_chars: usize) -> Viewport {
        let start = start.min(self.len_chars());
        let end = start.saturating_add(max_chars).min(self.len_chars());
        Viewport {
            chars: start..end,
            first_line: self.line_of(start),
            text: self.rope.slice(start..end).to_string(),
            revision: self.revision,
        }
    }
    /// Apply a bounded native input transaction; untouched document contents never cross the UI boundary.
    pub fn replace_viewport(&mut self, viewport: &Viewport, text: &str) -> Result<(), EditError> {
        if text == viewport.text {
            return Ok(());
        }
        let old = viewport.text.chars().collect::<Vec<_>>();
        let new = text.chars().collect::<Vec<_>>();
        let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
        let suffix = old[prefix..]
            .iter()
            .rev()
            .zip(new[prefix..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        self.apply(
            viewport.revision,
            vec![Edit {
                range: viewport.chars.start + prefix..viewport.chars.start + old.len() - suffix,
                insert: new[prefix..new.len() - suffix].iter().collect(),
            }],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_transaction_undo_and_original_crlf() {
        let original = "# 中文\r\n👩‍💻 café\r\n";
        let mut d = Document::new(original);
        let v = d.viewport(0, 100);
        d.replace_viewport(&v, "# 中文\r\n👩‍💻 cafés\r\n").unwrap();
        assert!(d.is_dirty());
        assert!(d.undo());
        assert_eq!(d.snapshot().to_string(), original);
        assert!(!d.is_dirty());
        assert!(d.redo());
        assert!(d.snapshot().to_string().contains("cafés"));
    }
    #[test]
    fn rejects_stale_and_overlapping_edits_without_mutation() {
        let mut d = Document::new("abcdef");
        assert!(
            d.apply(
                0,
                vec![
                    Edit {
                        range: 0..3,
                        insert: "x".into()
                    },
                    Edit {
                        range: 2..4,
                        insert: "y".into()
                    }
                ]
            )
            .is_err()
        );
        assert_eq!(d.snapshot().to_string(), "abcdef");
        d.apply(
            0,
            vec![Edit {
                range: 0..1,
                insert: "x".into(),
            }],
        )
        .unwrap();
        assert!(matches!(d.apply(0, vec![]), Err(EditError::StaleRevision)));
    }
    #[test]
    fn edits_in_different_windows_share_history() {
        let mut d = Document::new("a\nb\nc\nd\n");
        d.replace_viewport(&d.viewport(0, 2), "A\n").unwrap();
        d.replace_viewport(&d.viewport(6, 2), "D\n").unwrap();
        assert_eq!(d.snapshot().to_string(), "A\nb\nc\nD\n");
        d.undo();
        d.undo();
        assert_eq!(d.snapshot().to_string(), "a\nb\nc\nd\n");
    }
    #[test]
    fn long_line_viewport_is_bounded() {
        let mut d = Document::new(&"中".repeat(100_000));
        let v = d.viewport(50_000, 2048);
        assert_eq!(v.text.chars().count(), 2048);
        d.replace_viewport(&v, "hello").unwrap();
        assert_eq!(d.len_chars(), 100_000 - 2048 + 5);
    }
}
