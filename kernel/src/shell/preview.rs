use alloc::{boxed::Box, vec::Vec};
use core::str;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewKind {
    Text,
    Binary,
    InvalidUtf8,
}

pub struct Preview {
    bytes: Box<[u8]>,
    pub truncated: bool,
    kind: PreviewKind,
    text_end: usize,
}

impl Preview {
    pub(super) fn new(bytes: Vec<u8>, eof: bool) -> Self {
        let mut preview = Self {
            bytes: bytes.into_boxed_slice(),
            truncated: !eof,
            kind: PreviewKind::InvalidUtf8,
            text_end: 0,
        };
        let binary_char = |c: char| c.is_control() && !matches!(c, '\n' | '\r' | '\t');
        if preview
            .bytes
            .iter()
            .any(|&b| b.is_ascii() && binary_char(char::from(b)))
        {
            preview.kind = PreviewKind::Binary;
            return preview;
        }
        let text = match str::from_utf8(&preview.bytes) {
            Ok(text) => text,
            Err(error) if preview.truncated && error.error_len().is_none() => {
                // 只展示已验证的完整字符，保留原样本末尾的未完成字节。
                str::from_utf8(&preview.bytes[..error.valid_up_to()]).expect("valid UTF-8 prefix")
            }
            Err(_) => return preview,
        };
        if text.chars().any(binary_char) {
            preview.kind = PreviewKind::Binary;
        } else {
            preview.kind = PreviewKind::Text;
            preview.text_end = text.len();
        }
        preview
    }

    pub fn kind(&self) -> PreviewKind {
        self.kind
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn text(&self) -> Option<&str> {
        if self.kind == PreviewKind::Text {
            str::from_utf8(self.bytes.get(..self.text_end)?).ok()
        } else {
            None
        }
    }
}
