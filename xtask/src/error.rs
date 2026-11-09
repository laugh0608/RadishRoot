use std::{fmt, io};

#[derive(Debug)]
pub struct Error {
    pub kind: &'static str,
    pub detail: String,
    pub exit: i32,
}

impl Error {
    pub fn new(kind: &'static str, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
            exit: 1,
        }
    }
    pub fn usage(detail: impl Into<String>) -> Self {
        Self {
            kind: "usage",
            detail: detail.into(),
            exit: 2,
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.detail)
    }
}
impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::new("io", value.to_string())
    }
}
pub type Result<T> = std::result::Result<T, Error>;
