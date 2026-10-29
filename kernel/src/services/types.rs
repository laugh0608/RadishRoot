use super::path::{Path, PathError};
use crate::storage::{block, stream};
use alloc::{string::String, vec::Vec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootId(pub [u8; 16]);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolumeId(pub [u8; 16]);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperationId {
    pub boot: BootId,
    pub sequence: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileRef {
    pub boot: BootId,
    pub volume: VolumeId,
    pub path: Path,
    pub generation: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    File,
    Directory,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    pub reference: FileRef,
    pub name: String,
    pub kind: FileKind,
    pub size_bytes: Option<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MountState {
    Unchecked,
    Ready,
    Offline,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolumeInfo {
    pub id: VolumeId,
    pub capacity_bytes: u64,
    pub read_only: bool,
    pub flush_support: bool,
    pub mount_state: MountState,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestKind {
    List,
    Read,
    CreateText,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Validate,
    Mount,
    List,
    Read,
    Create,
    Write,
    FileSync,
    VolumeSync,
    Unmount,
    DeviceFlush,
    Complete,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Created,
    Validating,
    Running,
    Flushing,
    Succeeded,
    Failed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code {
    InvalidPath,
    InvalidArgument,
    StaleReference,
    ReadOnly,
    FlushUnsupported,
    NotFound,
    AlreadyExists,
    WrongKind,
    NoSpace,
    Corrupt,
    Io,
    Offline,
    ResourceLimit,
    IdentityExhausted,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FatCause {
    UnexpectedEof,
    WriteZero,
    InvalidInput,
    NotFound,
    AlreadyExists,
    DirectoryNotEmpty,
    Corrupt,
    NoSpace,
    NameLength,
    NameCharacter,
    Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    Path(PathError),
    Contract(Code),
    Fat(FatCause),
    Stream(stream::Error),
    Block(block::Error),
}
impl Cause {
    pub fn code(self) -> Code {
        match self {
            Self::Path(_) => Code::InvalidPath,
            Self::Contract(c) => c,
            Self::Fat(FatCause::NotFound) => Code::NotFound,
            Self::Fat(FatCause::AlreadyExists) => Code::AlreadyExists,
            Self::Fat(FatCause::NoSpace) => Code::NoSpace,
            Self::Fat(FatCause::Corrupt) => Code::Corrupt,
            Self::Fat(FatCause::InvalidInput) => Code::WrongKind,
            Self::Fat(FatCause::NameLength | FatCause::NameCharacter) => Code::InvalidPath,
            Self::Stream(stream::Error {
                cause: stream::Cause::Budget,
                ..
            }) => Code::ResourceLimit,
            Self::Fat(_) | Self::Stream(_) | Self::Block(_) => Code::Io,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PartialEffect {
    pub created: bool,
    pub written_bytes: usize,
    pub may_have_changed: bool,
    pub persistence_confirmed: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemError {
    pub code: Code,
    pub stage: Stage,
    pub object: Option<Path>,
    pub cause: Cause,
    pub partial_effect: PartialEffect,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationRecord {
    pub id: OperationId,
    pub request_kind: RequestKind,
    pub target: Option<Path>,
    pub stage: Stage,
    pub last_completed: Option<Stage>,
    pub status: Status,
    pub partial_effect: PartialEffect,
    pub error: Option<SystemError>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperationEvent {
    pub operation_id: OperationId,
    pub seq: u64,
    pub stage: Stage,
    pub status: Status,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeEvent {
    pub volume: VolumeId,
    pub paths: [Path; 2],
    pub generation: u64,
    pub operation_id: OperationId,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListCursor {
    pub directory: FileRef,
    pub offset: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListPage {
    pub entries: Vec<FileEntry>,
    pub next: Option<ListCursor>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Output {
    List(ListPage),
    Read { bytes: Vec<u8>, eof: bool },
    Created(FileRef),
}
pub struct Reply {
    pub operation_id: OperationId,
    pub result: Result<Output, SystemError>,
}
pub enum Request<'a> {
    List {
        path: &'a str,
        cursor: Option<&'a ListCursor>,
    },
    Read {
        path: &'a str,
        offset: u64,
        limit: usize,
        reference: Option<&'a FileRef>,
    },
    CreateText {
        path: &'a str,
        text: &'a str,
    },
}
impl Request<'_> {
    pub fn path(&self) -> &str {
        match self {
            Self::List { path, .. } | Self::Read { path, .. } | Self::CreateText { path, .. } => {
                path
            }
        }
    }
    pub fn kind(&self) -> RequestKind {
        match self {
            Self::List { .. } => RequestKind::List,
            Self::Read { .. } => RequestKind::Read,
            Self::CreateText { .. } => RequestKind::CreateText,
        }
    }
}
