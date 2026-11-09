use crate::storage::fat::{self, NameError};
use alloc::{string::String, vec::Vec};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Path(String);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathError {
    Empty,
    Length,
    Depth,
    Separator,
    OutsideVolume,
    Name(NameError),
}
impl Path {
    pub fn root() -> Self {
        Self(String::from("/data"))
    }
    pub fn resolve(&self, input: &str) -> Result<Self, PathError> {
        if input.is_empty() {
            return Err(PathError::Empty);
        }
        if input.len() > 1024 {
            return Err(PathError::Length);
        }
        if input.contains("//") {
            return Err(PathError::Separator);
        }
        let mut parts: Vec<&str> = if input.starts_with('/') {
            Vec::new()
        } else {
            self.relative()
                .split('/')
                .filter(|s| !s.is_empty())
                .collect()
        };
        let rest = if input.starts_with('/') {
            if input == "/data" || input == "/data/" {
                return Ok(Self::root());
            }
            input
                .strip_prefix("/data/")
                .ok_or(PathError::OutsideVolume)?
        } else {
            input
        };
        for part in rest.trim_end_matches('/').split('/') {
            match part {
                "." => (),
                ".." => {
                    parts.pop().ok_or(PathError::OutsideVolume)?;
                }
                name => {
                    fat::validate_component(name).map_err(PathError::Name)?;
                    parts.push(name);
                    if parts.len() > 8 {
                        return Err(PathError::Depth);
                    }
                }
            }
        }
        let joined = parts.join("/");
        if joined.len() + 6 > 1024 {
            return Err(PathError::Length);
        }
        Ok(if joined.is_empty() {
            Self::root()
        } else {
            Self(alloc::format!("/data/{joined}"))
        })
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn relative(&self) -> &str {
        self.0.strip_prefix("/data/").unwrap_or("")
    }
    pub fn parent(&self) -> Self {
        self.0
            .rsplit_once('/')
            .filter(|(p, _)| !p.is_empty())
            .map_or_else(Self::root, |(p, _)| Self(String::from(p)))
    }
    pub fn is_root(&self) -> bool {
        self.0 == "/data"
    }
}
