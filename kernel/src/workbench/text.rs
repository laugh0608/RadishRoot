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
}
impl Text {
    pub fn new(width: usize) -> Self {
        Self {
            lines: alloc::vec![String::new()],
            truncated: false,
            missing_glyphs: false,
            width: width.max(1),
            bytes: 0,
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
            if c == '\n' || self.lines.last().unwrap().len() == self.width {
                if self.lines.len() == ROWS || self.bytes >= BYTES {
                    self.truncated = true;
                    break;
                }
                self.lines.push(String::new());
                self.bytes += 1;
            }
            if c != '\n' {
                if self.bytes == BYTES {
                    self.truncated = true;
                    break;
                }
                self.lines.last_mut().unwrap().push(c);
                self.bytes += 1;
            }
        }
        Ok(())
    }
}
