//! 生产 FAT 后端：每请求一个挂载事务，失败先封锁 Stream 再析构句柄。
use super::*;
use crate::storage::{
    BlockDevice, Partition, fat, gpt,
    stream::{self, Phase, State, Stream},
};
use alloc::vec;
use fatfs::{FileSystem, FsOptions, Read, Seek, SeekFrom, Write};

mod writes;
use writes::Payload;

pub struct FatBackend<'a, D: BlockDevice> {
    pub(super) device: &'a mut D,
    layout: gpt::Layout,
    #[cfg(any(test, feature = "boot-test"))]
    failure: Option<Stage>,
    #[cfg(any(test, feature = "boot-test"))]
    sync_failure: Option<Stage>,
}
impl<'a, D: BlockDevice> FatBackend<'a, D> {
    pub fn new(device: &'a mut D, layout: gpt::Layout) -> Self {
        Self {
            device,
            layout,
            #[cfg(any(test, feature = "boot-test"))]
            failure: None,
            #[cfg(any(test, feature = "boot-test"))]
            sync_failure: None,
        }
    }
    #[cfg(any(test, feature = "boot-test"))]
    pub fn inject(&mut self, stage: Stage) {
        self.failure = Some(stage);
    }
    #[cfg(any(test, feature = "boot-test"))]
    pub fn inject_sync(&mut self, stage: Stage) {
        self.sync_failure = Some(stage);
    }
}
fn error(e: fatfs::Error<stream::Error>) -> Cause {
    use fatfs::Error::*;
    Cause::Fat(match e {
        Io(e) => return Cause::Stream(e),
        UnexpectedEof => FatCause::UnexpectedEof,
        WriteZero => FatCause::WriteZero,
        InvalidInput => FatCause::InvalidInput,
        NotFound => FatCause::NotFound,
        AlreadyExists => FatCause::AlreadyExists,
        DirectoryIsNotEmpty => FatCause::DirectoryNotEmpty,
        CorruptedFileSystem => FatCause::Corrupt,
        NotEnoughSpace => FatCause::NoSpace,
        InvalidFileNameLength => FatCause::NameLength,
        UnsupportedFileNameCharacter => FatCause::NameCharacter,
        _ => FatCause::Unknown,
    })
}
fn enter(progress: &mut Progress, state: &State, stage: Stage, phase: Phase, flushing: bool) {
    progress.enter(
        stage,
        if flushing {
            Status::Flushing
        } else {
            Status::Running
        },
    );
    state.phase(phase);
}
impl<D: BlockDevice> FatBackend<'_, D> {
    fn check_copy_identity(
        &mut self,
        source: &Path,
        destination: &Path,
        state: &State,
        progress: &mut Progress,
    ) -> Result<(), Cause> {
        enter(progress, state, Stage::Mount, Phase::Mount, false);
        let partition = Partition::open(self.device, &self.layout).map_err(Cause::Block)?;
        let mut stream = Stream::new(partition.read_only(), state);
        let fs = FileSystem::new(&mut stream, FsOptions::new()).map_err(error)?;
        if fs.fat_type() != fatfs::FatType::Fat32 {
            state.fail(stream::Cause::Aborted, 0);
            return Err(Cause::Fat(FatCause::Corrupt));
        }
        progress.mounted = true;
        progress.completed();
        enter(progress, state, Stage::CheckTarget, Phase::Read, false);
        let result = (|| {
            let root = fs.root_dir();
            drop(root.open_file(source.relative()).map_err(error)?);
            drop(root.open_file(destination.relative()).map_err(error)?);
            // 两个普通文件均已确认存在。锁定库按目录表项地址比较：同项返回 Ok，异项拒绝。
            // 单 BSP 独占保证探测与写入之间没有其他修改者。探测挂载只能读盘。
            match root.rename(source.relative(), &root, destination.relative()) {
                Ok(()) => Err(Cause::Contract(Code::SameFile)),
                Err(fatfs::Error::AlreadyExists) => Ok(()),
                Err(cause) => Err(error(cause)),
            }
        })();
        if let Err(cause) = result {
            let cause = state.first_error().map_or(cause, Cause::Stream);
            state.fail(stream::Cause::Aborted, 0);
            return Err(cause);
        }
        fs.unmount().map_err(error)?;
        state.check().map_err(Cause::Stream)?;
        progress.completed();
        Ok(())
    }
}

impl<D: BlockDevice> Backend for FatBackend<'_, D> {
    fn volume(&self) -> VolumeInfo {
        VolumeInfo {
            id: VolumeId(self.layout.data_guid.to_bytes()),
            capacity_bytes: self.layout.data().1 * 512,
            read_only: self.device.info().readonly || self.layout.readonly(),
            flush_support: self.device.info().flush,
            mount_state: MountState::Unchecked,
        }
    }
    fn run(
        &mut self,
        path: &Path,
        request: BackendRequest<'_>,
        progress: &mut Progress,
    ) -> Result<Data, Cause> {
        let syncing = matches!(request, BackendRequest::Sync);
        let writing = matches!(
            request,
            BackendRequest::CreateText(_)
                | BackendRequest::OverwriteText(_)
                | BackendRequest::CreateDirectory
                | BackendRequest::Remove
                | BackendRequest::Rename { .. }
                | BackendRequest::Copy { .. }
        ) || (syncing && !self.volume().read_only);
        let state = State::default();
        #[cfg(any(test, feature = "boot-test"))]
        let failure = if syncing {
            self.sync_failure.take()
        } else if writing {
            self.failure.take()
        } else {
            None
        };
        if let BackendRequest::Copy {
            source,
            overwrite: true,
        } = request
        {
            self.check_copy_identity(source, path, &state, progress)?;
        }
        enter(progress, &state, Stage::Mount, Phase::Mount, false);
        let partition = Partition::open(self.device, &self.layout).map_err(Cause::Block)?;
        let mut stream = Stream::new(
            if writing {
                partition
            } else {
                partition.read_only()
            },
            &state,
        );
        let fs = match FileSystem::new(&mut stream, FsOptions::new()) {
            Ok(fs) => fs,
            Err(e) => return Err(error(e)),
        };
        if fs.fat_type() != fatfs::FatType::Fat32 {
            state.fail(stream::Cause::Aborted, 0);
            drop(fs);
            return Err(Cause::Fat(FatCause::Corrupt));
        }
        progress.mounted = true;
        progress.completed();
        let result = (|| {
            let root = fs.root_dir();
            let data = match request {
                BackendRequest::Sync => Data::Synced,
                BackendRequest::List { offset } => {
                    enter(progress, &state, Stage::List, Phase::Read, false);
                    let dir = if path.is_root() {
                        root
                    } else {
                        root.open_dir(path.relative()).map_err(error)?
                    };
                    let mut entries = Vec::new();
                    let mut position = 0u32;
                    let mut next = None;
                    for (scanned, entry) in dir.iter().enumerate() {
                        if scanned >= MAX_ENTRIES as usize {
                            return Err(Cause::Contract(Code::ResourceLimit));
                        }
                        let e = entry.map_err(error)?;
                        let name = e.file_name();
                        if name == "." || name == ".." {
                            continue;
                        }
                        // 名称来自磁盘，仍须验证它可由公共路径层无损引用。
                        fat::validate_component(&name)
                            .map_err(|e| Cause::Path(super::path::PathError::Name(e)))?;
                        if position >= offset {
                            if entries.len() == PAGE_SIZE {
                                next = Some(position);
                                break;
                            }
                            entries.push(RawEntry {
                                name,
                                kind: if e.is_dir() {
                                    FileKind::Directory
                                } else {
                                    FileKind::File
                                },
                                size_bytes: if e.is_dir() { None } else { Some(e.len()) },
                            });
                        }
                        position += 1;
                    }
                    if position < offset {
                        return Err(Cause::Contract(Code::InvalidArgument));
                    }
                    Data::List { entries, next }
                }
                BackendRequest::Read { offset, limit } => {
                    enter(progress, &state, Stage::Read, Phase::Read, false);
                    let mut file = root.open_file(path.relative()).map_err(error)?;
                    if offset == 0 {
                        let mut bytes = vec![0; limit];
                        let mut count = 0;
                        while count < limit {
                            let read = file.read(&mut bytes[count..]).map_err(error)?;
                            if read == 0 {
                                break;
                            }
                            count += read;
                        }
                        bytes.truncate(count);
                        // SeekFrom::End 会遍历整个簇链。定位到样本后一字节以判定 EOF，
                        // 只检查样本边界的簇链，不从文件中额外读取数据字节。
                        let boundary = count as u64;
                        let next = file.seek(SeekFrom::Start(boundary + 1)).map_err(error)?;
                        if next < boundary || (count < limit && next > boundary) {
                            return Err(Cause::Fat(FatCause::UnexpectedEof));
                        }
                        bytes.shrink_to_fit();
                        Data::Read {
                            bytes,
                            eof: next == boundary,
                        }
                    } else {
                        // fatfs 将越过 EOF 的 seek 截至 EOF，先校验非零偏移。
                        let length = file.seek(SeekFrom::End(0)).map_err(error)?;
                        if offset > length {
                            return Err(Cause::Contract(Code::InvalidArgument));
                        }
                        file.seek(SeekFrom::Start(offset)).map_err(error)?;
                        let count = (length - offset).min(limit as u64) as usize;
                        let mut bytes = vec![0; count];
                        file.read_exact(&mut bytes).map_err(error)?;
                        Data::Read {
                            bytes,
                            eof: offset + count as u64 == length,
                        }
                    }
                }
                BackendRequest::LookupFile => {
                    enter(progress, &state, Stage::Read, Phase::Read, false);
                    drop(root.open_file(path.relative()).map_err(error)?);
                    state.check().map_err(Cause::Stream)?;
                    Data::File
                }
                BackendRequest::CreateText(text) | BackendRequest::OverwriteText(text) => {
                    writes::write_payload(
                        &root,
                        path,
                        Payload::Text(text),
                        matches!(request, BackendRequest::OverwriteText(_)),
                        &state,
                        progress,
                        #[cfg(any(test, feature = "boot-test"))]
                        failure,
                    )?
                }
                BackendRequest::Copy { source, overwrite } => writes::write_payload(
                    &root,
                    path,
                    Payload::File(source),
                    overwrite,
                    &state,
                    progress,
                    #[cfg(any(test, feature = "boot-test"))]
                    failure,
                )?,
                BackendRequest::CreateDirectory => {
                    enter(progress, &state, Stage::Create, Phase::Create, false);
                    fat::create_new_directory(&root, path.relative())
                        .map_err(|e| state.first_error().map_or_else(|| error(e), Cause::Stream))?;
                    progress.record.partial_effect.created = true;
                    Data::Created
                }
                BackendRequest::Remove => {
                    enter(progress, &state, Stage::Remove, Phase::Remove, false);
                    root.remove(path.relative())
                        .map_err(|e| state.first_error().map_or_else(|| error(e), Cause::Stream))?;
                    Data::Removed
                }
                BackendRequest::Rename { source } => {
                    enter(progress, &state, Stage::Rename, Phase::Rename, false);
                    fat::rename_file(&root, source.relative(), path.relative())
                        .map_err(|e| state.first_error().map_or_else(|| error(e), Cause::Stream))?;
                    Data::Renamed
                }
            };
            progress.completed();
            Ok(data)
        })();
        let result = match result {
            Err(e) => {
                state.fail(stream::Cause::Aborted, 0);
                drop(fs);
                Err(e)
            }
            Ok(data) => (|| {
                if writing {
                    enter(progress, &state, Stage::VolumeSync, Phase::VolumeSync, true);
                    #[cfg(any(test, feature = "boot-test"))]
                    if failure == Some(Stage::VolumeSync) {
                        // 即使本次卷没有 FSInfo 脏数据，也准确在卷同步边界注入一次失败。
                        if syncing {
                            state.fail(stream::Cause::InjectedWrite, 0);
                        } else {
                            state.inject_write_failure();
                        }
                    }
                    if let Err(e) = state.check() {
                        drop(fs);
                        return Err(Cause::Stream(e));
                    }
                    if let Err(e) = fs.flush() {
                        state.fail(stream::Cause::Aborted, 0);
                        drop(fs);
                        return Err(error(e));
                    }
                    state.check().map_err(Cause::Stream)?;
                    progress.completed();
                }
                enter(progress, &state, Stage::Unmount, Phase::Unmount, writing);
                fs.unmount().map_err(error)?;
                state.check().map_err(Cause::Stream)?;
                progress.completed();
                Ok(data)
            })(),
        };
        let result = result.and_then(|data| {
            if writing || syncing {
                enter(
                    progress,
                    &state,
                    Stage::DeviceFlush,
                    Phase::DeviceFlush,
                    true,
                );
                #[cfg(any(test, feature = "boot-test"))]
                if failure == Some(Stage::DeviceFlush) {
                    state.inject_flush_failure();
                }
                if syncing && stream.readonly() {
                    stream.sync_device().map_err(Cause::Stream)?;
                } else {
                    (&mut stream).flush().map_err(Cause::Stream)?;
                }
                state.check().map_err(Cause::Stream)?;
                progress.completed();
                progress.record.partial_effect.persistence_confirmed = true;
            }
            Ok(data)
        });
        progress.record.partial_effect.may_have_changed = state.writes() > 0;
        result
    }
}
