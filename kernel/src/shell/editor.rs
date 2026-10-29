use super::LINE_BYTES;
use alloc::{collections::VecDeque, string::String};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edit {
    Insert(char),
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    Previous,
    Next,
    Submit,
    Cancel,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Capacity,
    Control,
}
pub struct Editor {
    text: String,
    cursor: usize,
    history: VecDeque<String>,
    selected: Option<usize>,
    draft: String,
    pub evicted: u64,
}
impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}
impl Editor {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            history: VecDeque::new(),
            selected: None,
            draft: String::new(),
            evicted: 0,
        }
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn history_len(&self) -> usize {
        self.history.len()
    }
    fn previous_boundary(&self) -> usize {
        self.text[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i)
    }
    fn next_boundary(&self) -> usize {
        self.cursor
            + self.text[self.cursor..]
                .chars()
                .next()
                .map_or(0, char::len_utf8)
    }
    pub fn apply(&mut self, edit: Edit) -> Result<Option<String>, Error> {
        match edit {
            Edit::Insert(c) => {
                if c.is_control() {
                    return Err(Error::Control);
                }
                if self.text.len() + c.len_utf8() > LINE_BYTES {
                    return Err(Error::Capacity);
                }
                self.text.insert(self.cursor, c);
                self.cursor += c.len_utf8();
            }
            Edit::Backspace if self.cursor > 0 => {
                let at = self.previous_boundary();
                self.text.drain(at..self.cursor);
                self.cursor = at;
            }
            Edit::Delete if self.cursor < self.text.len() => {
                self.text.drain(self.cursor..self.next_boundary());
            }
            Edit::Left => self.cursor = self.previous_boundary(),
            Edit::Right => self.cursor = self.next_boundary(),
            Edit::Home => self.cursor = 0,
            Edit::End => self.cursor = self.text.len(),
            Edit::Previous => {
                if !self.history.is_empty() {
                    let at = if let Some(at) = self.selected {
                        at.saturating_sub(1)
                    } else {
                        self.draft = self.text.clone();
                        self.history.len() - 1
                    };
                    self.selected = Some(at);
                    self.text = self.history[at].clone();
                    self.cursor = self.text.len();
                }
            }
            Edit::Next => {
                if let Some(at) = self.selected {
                    if at + 1 < self.history.len() {
                        self.selected = Some(at + 1);
                        self.text = self.history[at + 1].clone()
                    } else {
                        self.selected = None;
                        self.text = core::mem::take(&mut self.draft)
                    }
                    self.cursor = self.text.len();
                }
            }
            Edit::Cancel => {
                self.text.clear();
                self.cursor = 0;
                self.selected = None;
                self.draft.clear();
            }
            Edit::Submit => {
                let text = core::mem::take(&mut self.text);
                self.cursor = 0;
                self.selected = None;
                self.draft.clear();
                if !text.trim().is_empty() {
                    if self.history.len() == 16 {
                        self.history.pop_front();
                        self.evicted = self.evicted.saturating_add(1)
                    }
                    self.history.push_back(text.clone());
                }
                return Ok(Some(text));
            }
            _ => (),
        }
        Ok(None)
    }
}
