//! 内置命令只转换为共享服务请求；串口和未来图形终端消费同一类型结果。
pub mod editor;
pub mod input;
pub mod parser;
pub mod render;
use crate::services::{self, path::Path, *};
use alloc::{string::String, vec::Vec};
use parser::{Command, ParseError};
pub const LINE_BYTES: usize = 1024;
pub const READ_BYTES: usize = 4096;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Parse(ParseError),
    Service(SystemError),
    ServiceIdentity(Code),
    NoNextPage,
    ForeignBoot,
    OperationUnavailable { evicted: u64 },
}
pub enum Output {
    Synced,
    PowerPrepared(services::power::Action),
    Help,
    Directory(Path),
    Files(ListPage),
    Bytes {
        bytes: Vec<u8>,
        eof: bool,
        offset: u64,
    },
    Created(FileRef),
    Removed(Path),
    Renamed {
        source: Path,
        target: FileRef,
    },
    Operations {
        records: Vec<OperationRecord>,
        evicted: u64,
    },
    Explanation {
        record: OperationRecord,
        events: Vec<OperationEvent>,
        events_evicted: u64,
    },
}
pub struct Reply {
    pub command_id: u64,
    pub operation_id: Option<OperationId>,
    pub result: Result<Output, Error>,
}
pub struct Shell {
    cwd: Path,
    next_command: u64,
    next_page: Option<(String, ListCursor)>,
}
impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}
impl Shell {
    pub fn new() -> Self {
        Self {
            cwd: Path::root(),
            next_command: 1,
            next_page: None,
        }
    }
    pub(crate) fn open_directory(&mut self, page: ListPage) {
        self.cwd = page.directory;
        self.next_page = None;
    }
    pub(crate) fn directory(&self) -> &Path {
        &self.cwd
    }
    pub fn execute<B: Backend>(
        &mut self,
        line: &str,
        files: &mut FileService<B>,
    ) -> Result<Reply, Code> {
        self.execute_parsed(parser::parse(line), files)
    }
    pub(crate) fn explain<B: Backend>(
        &mut self,
        id: OperationId,
        files: &mut FileService<B>,
    ) -> Result<Reply, Code> {
        self.execute_parsed(Ok(Command::Explain(id)), files)
    }
    fn execute_parsed<B: Backend>(
        &mut self,
        command: Result<Command, ParseError>,
        files: &mut FileService<B>,
    ) -> Result<Reply, Code> {
        if self.next_command == u64::MAX {
            return Err(Code::IdentityExhausted);
        }
        let command_id = self.next_command;
        self.next_command += 1;
        let mut operation_id = None;
        let result = (|| {
            let command = command.map_err(Error::Parse)?;
            let request = match command {
                Command::Power(action) => Request::Power(action),
                Command::Help => return Ok(Output::Help),
                Command::PrintDirectory => return Ok(Output::Directory(self.cwd.clone())),
                Command::Operations => {
                    return Ok(Output::Operations {
                        records: files.operations.iter().cloned().collect(),
                        evicted: files.operations.evicted,
                    });
                }
                Command::Explain(id) => {
                    if id.boot != files.boot_id() {
                        return Err(Error::ForeignBoot);
                    }
                    let record = files
                        .operations
                        .iter()
                        .find(|r| r.id == id)
                        .cloned()
                        .ok_or(Error::OperationUnavailable {
                            evicted: files.operations.evicted,
                        })?;
                    return Ok(Output::Explanation {
                        record,
                        events: files
                            .events
                            .iter()
                            .filter(|e| e.operation_id == id)
                            .copied()
                            .collect(),
                        events_evicted: files.events.evicted,
                    });
                }
                Command::List(ref path) => {
                    self.next_page = None;
                    Request::List { path, cursor: None }
                }
                Command::ChangeDirectory(ref path) => Request::List { path, cursor: None },
                Command::Next => {
                    let (path, cursor) = self.next_page.as_ref().ok_or(Error::NoNextPage)?;
                    Request::List {
                        path,
                        cursor: Some(cursor),
                    }
                }
                Command::Write { ref path, ref text } => Request::CreateText { path, text },
                Command::MakeDirectory(ref path) => Request::CreateDirectory { path },
                Command::Remove(ref path) => Request::Remove {
                    path,
                    reference: None,
                },
                Command::Rename {
                    ref source,
                    ref destination,
                } => Request::Rename {
                    source,
                    destination,
                    reference: None,
                },
                Command::Cat { ref path, offset } => Request::Read {
                    path,
                    offset,
                    limit: READ_BYTES,
                    reference: None,
                },
            };
            let r = files
                .execute(&self.cwd, request)
                .map_err(Error::ServiceIdentity)?;
            operation_id = Some(r.operation_id);
            let result = r.result.map_err(Error::Service)?;
            Ok(match result {
                services::Output::Synced => Output::Synced,
                services::Output::PowerPrepared(a) => Output::PowerPrepared(a),
                services::Output::List(page) if matches!(command, Command::ChangeDirectory(_)) => {
                    self.open_directory(page);
                    Output::Directory(self.cwd.clone())
                }
                services::Output::List(page) => {
                    self.next_page = page
                        .next
                        .as_ref()
                        .map(|c| (String::from(c.directory.path.as_str()), c.clone()));
                    Output::Files(page)
                }
                services::Output::Read { bytes, eof } => Output::Bytes {
                    bytes,
                    eof,
                    offset: match command {
                        Command::Cat { offset, .. } => offset,
                        _ => unreachable!(),
                    },
                },
                services::Output::Created(reference) => Output::Created(reference),
                services::Output::Removed(path) => Output::Removed(path),
                services::Output::Renamed { source, target } => Output::Renamed { source, target },
            })
        })();
        Ok(Reply {
            command_id,
            operation_id,
            result,
        })
    }
}
#[cfg(test)]
mod tests;
