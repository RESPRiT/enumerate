use std::path::PathBuf;

use anyhow::Result;

use crate::doc::{Doc, DECISION_COLUMN, Warning};

const UNDO_STACK_MAX: usize = 500;

#[derive(Clone)]
struct UndoEntry {
    group_idx: usize,
    case_idx: usize,
    value: String,
    input_cursor: usize,
}

pub struct App {
    pub file_path: PathBuf,
    pub doc: Doc,
    pub warnings: Vec<Warning>,
    /// Flat list of selectable Status cells: (group_index, case_index).
    pub selectable: Vec<(usize, usize)>,
    /// Cursor position. Values in `0..selectable.len()` are cells; `selectable.len()` is Submit.
    pub cursor: usize,
    /// Byte offset of the input cursor within the currently-selected Status field.
    /// Always on a char boundary; meaningless when `is_on_submit()`.
    pub input_cursor: usize,
    pub scroll: u16,
    pub quit: bool,
    pub dirty: bool,
    undo_stack: Vec<UndoEntry>,
}

impl App {
    pub fn new(file_path: PathBuf, doc: Doc, warnings: Vec<Warning>) -> Self {
        let mut selectable = Vec::new();
        for (gi, group) in doc.groups.iter().enumerate() {
            for ci in 0..group.cases.len() {
                selectable.push((gi, ci));
            }
        }
        let mut app = Self {
            file_path,
            doc,
            warnings,
            selectable,
            cursor: 0,
            input_cursor: 0,
            scroll: 0,
            quit: false,
            dirty: false,
            undo_stack: Vec::new(),
        };
        app.reset_input_cursor();
        app
    }

    pub fn submit_index(&self) -> usize {
        self.selectable.len()
    }

    pub fn is_on_submit(&self) -> bool {
        self.cursor >= self.submit_index()
    }

    pub fn selected_case(&self) -> Option<(usize, usize)> {
        if self.is_on_submit() {
            None
        } else {
            self.selectable.get(self.cursor).copied()
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor < self.submit_index() {
            self.cursor += 1;
            self.reset_input_cursor();
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.reset_input_cursor();
        }
    }

    fn reset_input_cursor(&mut self) {
        self.input_cursor = self
            .selected_case()
            .and_then(|(gi, ci)| self.doc.groups[gi].cases[ci].fields.get(DECISION_COLUMN))
            .map(|s| s.len())
            .unwrap_or(0);
    }

    pub fn append_status_char(&mut self, c: char) -> bool {
        let Some((gi, ci)) = self.selected_case() else {
            return false;
        };
        self.snapshot();
        let value = self.doc.groups[gi].cases[ci]
            .fields
            .entry(DECISION_COLUMN.to_string())
            .or_default();
        let pos = self.input_cursor.min(value.len());
        value.insert(pos, c);
        self.input_cursor = pos + c.len_utf8();
        self.dirty = true;
        true
    }

    pub fn backspace_status(&mut self) -> bool {
        let Some((gi, ci)) = self.selected_case() else {
            return false;
        };
        let Some(value) = self.doc.groups[gi].cases[ci].fields.get_mut(DECISION_COLUMN) else {
            return false;
        };
        if self.input_cursor == 0 || value.is_empty() {
            return false;
        }
        let prev = value[..self.input_cursor]
            .char_indices()
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.snapshot();
        let value = self.doc.groups[gi].cases[ci]
            .fields
            .get_mut(DECISION_COLUMN)
            .expect("field exists");
        value.replace_range(prev..self.input_cursor, "");
        self.input_cursor = prev;
        self.dirty = true;
        true
    }

    pub fn cursor_to_line_start(&mut self) {
        if self.selected_case().is_some() {
            self.input_cursor = 0;
        }
    }

    pub fn cursor_to_line_end(&mut self) {
        if let Some((gi, ci)) = self.selected_case() {
            self.input_cursor = self.doc.groups[gi].cases[ci]
                .fields
                .get(DECISION_COLUMN)
                .map(|s| s.len())
                .unwrap_or(0);
        }
    }

    pub fn cursor_left(&mut self) {
        let Some((gi, ci)) = self.selected_case() else {
            return;
        };
        let value = self.doc.groups[gi].cases[ci]
            .fields
            .get(DECISION_COLUMN)
            .map(String::as_str)
            .unwrap_or("");
        if self.input_cursor == 0 {
            return;
        }
        let prev = value[..self.input_cursor]
            .char_indices()
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.input_cursor = prev;
    }

    pub fn cursor_right(&mut self) {
        let Some((gi, ci)) = self.selected_case() else {
            return;
        };
        let value = self.doc.groups[gi].cases[ci]
            .fields
            .get(DECISION_COLUMN)
            .map(String::as_str)
            .unwrap_or("");
        if self.input_cursor >= value.len() {
            return;
        }
        let next = value[self.input_cursor..]
            .char_indices()
            .nth(1)
            .map(|(i, _)| self.input_cursor + i)
            .unwrap_or(value.len());
        self.input_cursor = next;
    }

    pub fn kill_to_line_end(&mut self) -> bool {
        let Some((gi, ci)) = self.selected_case() else {
            return false;
        };
        {
            let Some(value) = self.doc.groups[gi].cases[ci].fields.get(DECISION_COLUMN) else {
                return false;
            };
            if self.input_cursor >= value.len() {
                return false;
            }
        }
        self.snapshot();
        let value = self.doc.groups[gi].cases[ci]
            .fields
            .get_mut(DECISION_COLUMN)
            .expect("field exists");
        value.truncate(self.input_cursor);
        self.dirty = true;
        true
    }

    fn snapshot(&mut self) {
        let Some((gi, ci)) = self.selected_case() else {
            return;
        };
        let value = self.doc.groups[gi].cases[ci]
            .fields
            .get(DECISION_COLUMN)
            .cloned()
            .unwrap_or_default();
        self.undo_stack.push(UndoEntry {
            group_idx: gi,
            case_idx: ci,
            value,
            input_cursor: self.input_cursor,
        });
        if self.undo_stack.len() > UNDO_STACK_MAX {
            let drop = self.undo_stack.len() - UNDO_STACK_MAX;
            self.undo_stack.drain(0..drop);
        }
    }

    pub fn undo(&mut self) -> bool {
        let Some(entry) = self.undo_stack.pop() else {
            return false;
        };
        if let Some(idx) = self
            .selectable
            .iter()
            .position(|&c| c == (entry.group_idx, entry.case_idx))
        {
            self.cursor = idx;
        }
        let case = &mut self.doc.groups[entry.group_idx].cases[entry.case_idx];
        if entry.value.is_empty() {
            case.fields.insert(DECISION_COLUMN.to_string(), String::new());
        } else {
            case.fields
                .insert(DECISION_COLUMN.to_string(), entry.value.clone());
        }
        self.input_cursor = entry.input_cursor;
        self.dirty = true;
        true
    }

    pub fn save(&mut self) -> Result<()> {
        let output = crate::doc::serialize(&self.doc);
        std::fs::write(&self.file_path, output)?;
        self.dirty = false;
        Ok(())
    }

    pub fn total_cases(&self) -> usize {
        self.doc.total_cases()
    }

    pub fn filled_cases(&self) -> usize {
        self.doc.filled_cases()
    }
}
