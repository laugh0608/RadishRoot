use super::LINE_BYTES;
use crate::services::{BootId, OperationId};
use alloc::{string::String, vec::Vec};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    Length,
    Arguments,
    Quote,
    Escape,
    Control,
    Operator,
    UnknownCommand,
    Number,
    Identity,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Help,
    List(String),
    Next,
    Write { path: String, text: String },
    Cat { path: String, offset: u64 },
    Operations,
    Explain(OperationId),
}
pub struct CommandSpec {
    pub name: &'static str,
    pub usage: &'static str,
    pub modifies: bool,
    pub result: &'static str,
}
pub const COMMANDS: [CommandSpec; 6] = [
    CommandSpec {
        name: "help",
        usage: "help",
        modifies: false,
        result: "help",
    },
    CommandSpec {
        name: "ls",
        usage: "ls [path] | ls --next",
        modifies: false,
        result: "files",
    },
    CommandSpec {
        name: "write",
        usage: "write <path> <text> (create only)",
        modifies: true,
        result: "created",
    },
    CommandSpec {
        name: "cat",
        usage: "cat <path> [offset] (up to 4096 bytes)",
        modifies: false,
        result: "bytes",
    },
    CommandSpec {
        name: "operations",
        usage: "operations",
        modifies: false,
        result: "operations",
    },
    CommandSpec {
        name: "explain",
        usage: "explain <boot-hex:sequence>",
        modifies: false,
        result: "explanation",
    },
];
fn words(line: &str) -> Result<Vec<String>, ParseError> {
    if line.len() > LINE_BYTES {
        return Err(ParseError::Length);
    }
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut started = false;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c.is_control() && c != '\t' {
            return Err(ParseError::Control);
        }
        if c == '\\' && quote != Some('\'') {
            let escaped = match chars.next().ok_or(ParseError::Escape)? {
                '\\' => '\\',
                '"' => '"',
                '\'' => '\'',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                _ => return Err(ParseError::Escape),
            };
            word.push(escaped);
            started = true;
            continue;
        }
        if let Some(q) = quote {
            if c == q {
                quote = None
            } else {
                word.push(c)
            };
            continue;
        }
        if c == '\'' || c == '"' {
            quote = Some(c);
            started = true;
            continue;
        }
        if c == ' ' || c == '\t' {
            if started {
                words.push(core::mem::take(&mut word));
                started = false;
                if words.len() > 8 {
                    return Err(ParseError::Arguments);
                }
            }
        } else {
            if matches!(c, '|' | ';' | '&' | '<' | '>' | '$' | '`') {
                return Err(ParseError::Operator);
            }
            word.push(c);
            started = true
        }
    }
    if quote.is_some() {
        return Err(ParseError::Quote);
    }
    if started {
        words.push(word)
    }
    if words.len() > 8 {
        return Err(ParseError::Arguments);
    }
    if words.is_empty() {
        return Err(ParseError::Empty);
    }
    Ok(words)
}
fn number(text: &str) -> Result<u64, ParseError> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ParseError::Number);
    }
    text.parse().map_err(|_| ParseError::Number)
}
fn identity(text: &str) -> Result<OperationId, ParseError> {
    let (boot, sequence) = text.split_once(':').ok_or(ParseError::Identity)?;
    if boot.len() != 32 || !boot.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(ParseError::Identity);
    }
    let id = u128::from_str_radix(boot, 16).map_err(|_| ParseError::Identity)?;
    let sequence = number(sequence)?;
    if id == 0 || sequence == 0 {
        return Err(ParseError::Identity);
    }
    Ok(OperationId {
        boot: BootId(id.to_be_bytes()),
        sequence,
    })
}
pub fn parse(line: &str) -> Result<Command, ParseError> {
    let w = words(line)?;
    let a: Vec<&str> = w.iter().map(String::as_str).collect();
    Ok(match a.as_slice() {
        ["help"] => Command::Help,
        ["ls"] => Command::List(String::from("/data")),
        ["ls", "--next"] => Command::Next,
        ["ls", path] => Command::List(String::from(*path)),
        ["write", path, text] => Command::Write {
            path: String::from(*path),
            text: String::from(*text),
        },
        ["cat", path] => Command::Cat {
            path: String::from(*path),
            offset: 0,
        },
        ["cat", path, offset] => Command::Cat {
            path: String::from(*path),
            offset: number(offset)?,
        },
        ["operations"] => Command::Operations,
        ["explain", id] => Command::Explain(identity(id)?),
        [name, ..] if COMMANDS.iter().any(|s| s.name == *name) => {
            return Err(ParseError::Arguments);
        }
        _ => return Err(ParseError::UnknownCommand),
    })
}
