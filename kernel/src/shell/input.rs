//! 有界 UART 行解码；输入损坏后丢弃整行，不能提交剩余文本。
use super::editor::{Edit, Editor};
use alloc::string::String;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputError {
    Utf8,
    Control,
    Escape,
    Capacity,
    Transport,
}
pub enum Action {
    None,
    Changed,
    Submitted(String),
    Rejected(InputError),
}
pub struct Input {
    pub editor: Editor,
    utf8: [u8; 4],
    used: usize,
    needed: usize,
    escape: u8,
    poisoned: Option<InputError>,
    skip_lf: bool,
}
impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}
impl Input {
    pub fn new() -> Self {
        Self {
            editor: Editor::new(),
            utf8: [0; 4],
            used: 0,
            needed: 0,
            escape: 0,
            poisoned: None,
            skip_lf: false,
        }
    }
    pub fn pending_escape(&self) -> bool {
        self.escape != 0
    }
    fn reject(&mut self, e: InputError) -> Action {
        self.poisoned = Some(e);
        self.used = 0;
        self.needed = 0;
        self.escape = 0;
        Action::Rejected(e)
    }
    pub fn transport_error(&mut self) -> Action {
        self.reject(InputError::Transport)
    }
    fn edit(&mut self, e: Edit) -> Action {
        match self.editor.apply(e) {
            Ok(Some(line)) => Action::Submitted(line),
            Ok(None) => Action::Changed,
            Err(_) => self.reject(InputError::Capacity),
        }
    }
    pub fn cancel(&mut self) -> Action {
        self.used = 0;
        self.needed = 0;
        self.escape = 0;
        self.poisoned = None;
        self.edit(Edit::Cancel)
    }
    pub fn escape_timeout(&mut self) -> Action {
        if self.escape == 1 {
            self.cancel()
        } else if self.escape != 0 {
            self.reject(InputError::Escape)
        } else {
            Action::None
        }
    }
    pub fn receive(&mut self, b: u8) -> Action {
        if b == b'\n' && self.skip_lf {
            self.skip_lf = false;
            return Action::None;
        }
        self.skip_lf = false;
        if b == 3 {
            return self.cancel();
        }
        if let Some(error) = self.poisoned {
            if matches!(b, b'\r' | b'\n') {
                self.cancel();
                self.skip_lf = b == b'\r';
                return Action::Rejected(error);
            }
            return Action::None;
        }
        if self.escape != 0 {
            let state = self.escape;
            self.escape = 0;
            let edit = match (state, b) {
                (1, b'[') => {
                    self.escape = 2;
                    return Action::None;
                }
                (2, b'3') => {
                    self.escape = 3;
                    return Action::None;
                }
                (2, b'A') => Edit::Previous,
                (2, b'B') => Edit::Next,
                (2, b'C') => Edit::Right,
                (2, b'D') => Edit::Left,
                (2, b'H') => Edit::Home,
                (2, b'F') => Edit::End,
                (3, b'~') => Edit::Delete,
                _ => return self.reject(InputError::Escape),
            };
            return self.edit(edit);
        }
        if self.used != 0 {
            if b & 0xc0 != 0x80 {
                let action = self.reject(InputError::Utf8);
                if matches!(b, b'\r' | b'\n') {
                    self.cancel();
                    self.skip_lf = b == b'\r';
                }
                return action;
            }
            self.utf8[self.used] = b;
            self.used += 1;
            if self.used < self.needed {
                return Action::None;
            }
            self.used = 0;
            let Ok(text) = core::str::from_utf8(&self.utf8[..self.needed]) else {
                return self.reject(InputError::Utf8);
            };
            let c = text.chars().next().unwrap();
            if c.is_control() {
                return self.reject(InputError::Control);
            }
            return self.edit(Edit::Insert(c));
        }
        match b {
            b'\r' | b'\n' => {
                self.skip_lf = b == b'\r';
                self.edit(Edit::Submit)
            }
            27 => {
                self.escape = 1;
                Action::None
            }
            8 | 127 => self.edit(Edit::Backspace),
            1 => self.edit(Edit::Home),
            5 => self.edit(Edit::End),
            2 => self.edit(Edit::Left),
            6 => self.edit(Edit::Right),
            16 => self.edit(Edit::Previous),
            14 => self.edit(Edit::Next),
            4 => self.edit(Edit::Delete),
            32..=126 => self.edit(Edit::Insert(b as char)),
            0xc2..=0xdf => {
                self.utf8[0] = b;
                self.used = 1;
                self.needed = 2;
                Action::None
            }
            0xe0..=0xef => {
                self.utf8[0] = b;
                self.used = 1;
                self.needed = 3;
                Action::None
            }
            0xf0..=0xf4 => {
                self.utf8[0] = b;
                self.used = 1;
                self.needed = 4;
                Action::None
            }
            0..=31 => self.reject(InputError::Control),
            _ => self.reject(InputError::Utf8),
        }
    }
}
