//! 有界显示投影；缺字只改变显示，不改领域字符串或原始字节。
use alloc::{string::String, vec::Vec};
use core::fmt::{self, Write};
pub const ROWS: usize = 512;
pub const BYTES: usize = 64 * 1024;
pub struct Text {
    pub lines: Vec<String>,
    pub truncated: bool,
    pub missing_glyphs: bool,
    width: usize,
    bytes: usize,
    skip: usize,
    rows: usize,
    row: usize,
    column: usize,
}
impl Text {
    pub fn new(width: usize) -> Self {
        Self::window(width, 0, ROWS)
    }
    pub(super) fn window(width: usize, skip: usize, rows: usize) -> Self {
        Self {
            lines: if skip == 0 {
                alloc::vec![String::new()]
            } else {
                Vec::new()
            },
            truncated: false,
            missing_glyphs: false,
            width: width.max(1),
            bytes: 0,
            skip,
            rows: rows.max(1),
            row: 0,
            column: 0,
        }
    }
    pub(super) fn last_row(&self) -> usize {
        self.row
    }
    pub(super) fn preview_content(&mut self, content: &str) {
        let mut parts = content.split('\r');
        if let Some(first) = parts.next() {
            let _ = self.write_str(first);
        }
        for part in parts {
            if self.truncated {
                break;
            }
            let _ = self.write_char('\n');
            let _ = self.write_str(part.strip_prefix('\n').unwrap_or(part));
        }
    }
}
impl Write for Text {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            if self.truncated {
                break;
            }
            let c = match c {
                '\n' => '\n',
                ' '..='~' => c,
                '\t' => ' ',
                _ => {
                    self.missing_glyphs = true;
                    '?'
                }
            };
            if c == '\n' || self.column == self.width {
                if self.lines.len() == self.rows || self.bytes >= BYTES {
                    self.truncated = true;
                    break;
                }
                self.row += 1;
                self.column = 0;
                if self.row >= self.skip {
                    self.bytes += usize::from(!self.lines.is_empty());
                    self.lines.push(String::new());
                }
            }
            if c != '\n' {
                self.column += 1;
                if let Some(line) = self.lines.last_mut() {
                    if self.bytes == BYTES {
                        self.truncated = true;
                        break;
                    }
                    line.push(c);
                    self.bytes += 1;
                }
            }
        }
        Ok(())
    }
}
