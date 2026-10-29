//! 唯一文件语义入口。单 BSP 同步请求；真实后端只通过 DATA Partition 访问设备。
pub mod fat;
pub mod path;
mod types;
use alloc::{collections::VecDeque, string::String, vec::Vec};
use path::Path;
pub use types::*;

pub const PAGE_SIZE: usize = 32;
pub const MAX_BYTES: usize = 64 * 1024;
pub const MAX_ENTRIES: u32 = 4096;
const RECORDS: usize = 32;
const EVENTS: usize = 128;
const CHANGES: usize = 32;

pub struct History<T> {
    entries: VecDeque<T>,
    capacity: usize,
    pub evicted: u64,
}
impl<T> History<T> {
    fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            capacity,
            evicted: 0,
        }
    }
    fn push(&mut self, item: T) {
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
            self.evicted = self.evicted.saturating_add(1);
        }
        self.entries.push_back(item);
    }
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.entries.iter()
    }
}
pub struct Progress {
    pub record: OperationRecord,
    pub mounted: bool,
    transitions: Vec<(Stage, Status)>,
}
impl Progress {
    pub fn enter(&mut self, stage: Stage, status: Status) {
        self.record.stage = stage;
        self.record.status = status;
        assert!(
            self.transitions.len() < 16,
            "backend exceeded the operation stage bound"
        );
        self.transitions.push((stage, status));
    }
    pub fn completed(&mut self) {
        self.record.last_completed = Some(self.record.stage);
    }
}
pub struct RawEntry {
    pub name: String,
    pub kind: FileKind,
    pub size_bytes: Option<u64>,
}
pub enum Data {
    List {
        entries: Vec<RawEntry>,
        next: Option<u32>,
    },
    Read {
        bytes: Vec<u8>,
        eof: bool,
    },
    Created,
}
pub enum BackendRequest<'a> {
    List { offset: u32 },
    Read { offset: u64, limit: usize },
    CreateText(&'a str),
}
pub trait Backend {
    fn volume(&self) -> VolumeInfo;
    fn run(
        &mut self,
        path: &Path,
        request: BackendRequest<'_>,
        progress: &mut Progress,
    ) -> Result<Data, Cause>;
}
pub struct FileService<B: Backend> {
    backend: B,
    boot: BootId,
    volume: VolumeInfo,
    generation: u64,
    next_operation: u64,
    next_event: u64,
    pub operations: History<OperationRecord>,
    pub events: History<OperationEvent>,
    pub changes: History<ChangeEvent>,
}
impl<B: Backend> FileService<B> {
    pub fn new(backend: B, boot: BootId) -> Result<Self, Code> {
        let volume = backend.volume();
        if boot.0 == [0; 16] || volume.id.0 == [0; 16] {
            return Err(Code::InvalidArgument);
        }
        Ok(Self {
            backend,
            boot,
            volume,
            generation: 0,
            next_operation: 1,
            next_event: 1,
            operations: History::new(RECORDS),
            events: History::new(EVENTS),
            changes: History::new(CHANGES),
        })
    }
    pub fn boot_id(&self) -> BootId {
        self.boot
    }
    pub fn volume(&self) -> VolumeInfo {
        self.volume
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    fn reference(&self, path: Path) -> FileRef {
        FileRef {
            boot: self.boot,
            volume: self.volume.id,
            path,
            generation: self.generation,
        }
    }
    fn current(&self, reference: &FileRef, path: &Path) -> bool {
        reference == &self.reference(path.clone())
    }
    fn validate<'a>(
        &self,
        path: &Path,
        request: &'a Request<'a>,
    ) -> Result<BackendRequest<'a>, Cause> {
        let fail = |code| Cause::Contract(code);
        if self.volume.mount_state == MountState::Offline {
            return Err(fail(Code::Offline));
        }
        match request {
            Request::List { cursor, .. } => {
                if cursor.is_some_and(|c| !self.current(&c.directory, path)) {
                    return Err(fail(Code::StaleReference));
                }
                let offset = cursor.map_or(0, |c| c.offset);
                if offset >= MAX_ENTRIES {
                    return Err(fail(Code::ResourceLimit));
                }
                Ok(BackendRequest::List { offset })
            }
            Request::Read {
                offset,
                limit,
                reference,
                ..
            } => {
                if reference.is_some_and(|r| !self.current(r, path)) {
                    return Err(fail(Code::StaleReference));
                }
                if *limit == 0 || *limit > MAX_BYTES || offset.checked_add(*limit as u64).is_none()
                {
                    return Err(fail(Code::InvalidArgument));
                }
                if path.is_root() {
                    return Err(fail(Code::WrongKind));
                }
                Ok(BackendRequest::Read {
                    offset: *offset,
                    limit: *limit,
                })
            }
            Request::CreateText { text, .. } => {
                if path.is_root() {
                    return Err(fail(Code::AlreadyExists));
                }
                if text.len() > MAX_BYTES {
                    return Err(fail(Code::ResourceLimit));
                }
                if self.volume.read_only {
                    return Err(fail(Code::ReadOnly));
                }
                if !self.volume.flush_support {
                    return Err(fail(Code::FlushUnsupported));
                }
                if self.generation == u64::MAX {
                    return Err(fail(Code::IdentityExhausted));
                }
                Ok(BackendRequest::CreateText(text))
            }
        }
    }
    pub fn execute(&mut self, cwd: &Path, request: Request<'_>) -> Result<Reply, Code> {
        // 本后端每请求最多 16 个阶段；预留空间，耗尽时在 IO 前拒绝，不允许回绕。
        if self.next_operation == u64::MAX || self.next_event > u64::MAX - 32 {
            return Err(Code::IdentityExhausted);
        }
        let id = OperationId {
            boot: self.boot,
            sequence: self.next_operation,
        };
        self.next_operation += 1;
        let mut progress = Progress {
            record: OperationRecord {
                id,
                request_kind: request.kind(),
                target: None,
                stage: Stage::Validate,
                last_completed: None,
                status: Status::Created,
                partial_effect: PartialEffect::default(),
                error: None,
            },
            mounted: false,
            transitions: Vec::new(),
        };
        progress.enter(Stage::Validate, Status::Created);
        progress.enter(Stage::Validate, Status::Validating);
        let result = (|| {
            let path = cwd.resolve(request.path()).map_err(Cause::Path)?;
            progress.record.target = Some(path.clone());
            let action = self.validate(&path, &request)?;
            progress.completed();
            let data = self.backend.run(&path, action, &mut progress)?;
            if request.kind() == RequestKind::CreateText
                && !progress.record.partial_effect.persistence_confirmed
            {
                return Err(Cause::Contract(Code::Io));
            }
            Ok(data)
        })();
        if progress.mounted {
            self.volume.mount_state = MountState::Ready;
        }
        let effect = progress.record.partial_effect;
        if effect.may_have_changed || effect.created {
            self.generation += 1; // 创建请求在 IO 前已拒绝耗尽的代次。
            let path = progress
                .record
                .target
                .as_ref()
                .expect("effect requires a validated path");
            self.changes.push(ChangeEvent {
                volume: self.volume.id,
                paths: [path.clone(), path.parent()],
                generation: self.generation,
                operation_id: id,
            });
        }
        let result = result.and_then(|data| {
            let path = progress
                .record
                .target
                .as_ref()
                .expect("successful request has path");
            Ok(match data {
                Data::Created => Output::Created(self.reference(path.clone())),
                Data::Read { bytes, eof } => Output::Read { bytes, eof },
                Data::List { entries, next } => {
                    let mut files = Vec::new();
                    for e in entries {
                        let child = path.resolve(&e.name).map_err(Cause::Path)?;
                        files.push(FileEntry {
                            reference: self.reference(child),
                            name: e.name,
                            kind: e.kind,
                            size_bytes: e.size_bytes,
                        });
                    }
                    Output::List(ListPage {
                        entries: files,
                        next: next.map(|offset| ListCursor {
                            directory: self.reference(path.clone()),
                            offset,
                        }),
                    })
                }
            })
        });
        let result = match result {
            Ok(output) => {
                progress.enter(Stage::Complete, Status::Succeeded);
                progress.completed();
                Ok(output)
            }
            Err(cause) => {
                let error = SystemError {
                    code: cause.code(),
                    stage: progress.record.stage,
                    object: progress.record.target.clone(),
                    cause,
                    partial_effect: effect,
                };
                if effect.may_have_changed
                    || matches!(error.code, Code::Io | Code::Corrupt)
                    || matches!(cause, Cause::Stream(_))
                {
                    self.volume.mount_state = MountState::Offline;
                }
                progress.enter(error.stage, Status::Failed);
                progress.record.error = Some(error.clone());
                Err(error)
            }
        };
        for (stage, status) in &progress.transitions {
            self.events.push(OperationEvent {
                operation_id: id,
                seq: self.next_event,
                stage: *stage,
                status: *status,
            });
            self.next_event += 1;
        }
        self.operations.push(progress.record);
        Ok(Reply {
            operation_id: id,
            result,
        })
    }
}
#[cfg(test)]
pub(crate) mod tests;
