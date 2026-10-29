//! 生产 FAT 后端：每请求一个挂载事务，失败先封锁 Stream 再析构句柄。
use super::*;
use crate::storage::{
    BlockDevice, Partition, fat, gpt,
    stream::{self, Phase, State, Stream},
};
use alloc::vec;
use fatfs::{FileSystem, FsOptions, Read, Seek, SeekFrom, Write};

pub struct FatBackend<'a, D: BlockDevice> {
    pub(super) device: &'a mut D,
    layout: gpt::Layout,
    #[cfg(any(test, feature = "boot-test"))]
    failure: Option<Stage>,
}
impl<'a, D: BlockDevice> FatBackend<'a, D> {
    pub fn new(device: &'a mut D, layout: gpt::Layout) -> Self {
        Self {
            device,
            layout,
            #[cfg(any(test, feature = "boot-test"))]
            failure: None,
        }
    }
    #[cfg(any(test, feature = "boot-test"))]
    pub fn inject(&mut self, stage: Stage) {
        self.failure = Some(stage);
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
        let writing = matches!(request, BackendRequest::CreateText(_));
        let state = State::default();
        #[cfg(any(test, feature = "boot-test"))]
        let failure = self.failure.take();
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
                    // fatfs 将越过 EOF 的 seek 截至 EOF，因此必须先独立取得大小并校验 offset。
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
                BackendRequest::CreateText(text) => {
                    enter(progress, &state, Stage::Create, Phase::Create, false);
                    let mut file = match fat::create_new(&root, path.relative()) {
                        Ok(f) => f,
                        Err(e) => {
                            let cause = state.first_error().map_or_else(|| error(e), Cause::Stream);
                            state.fail(stream::Cause::Aborted, 0);
                            return Err(cause);
                        }
                    };
                    progress.record.partial_effect.created = true;
                    progress.completed();
                    // 所有文件错误在 file Drop 之前锁存；保留原始 FAT 错误。
                    let write_result = (|| {
                        enter(progress, &state, Stage::Write, Phase::Write, false);
                        while progress.record.partial_effect.written_bytes < text.len() {
                            let at = progress.record.partial_effect.written_bytes;
                            let n = file.write(&text.as_bytes()[at..]).map_err(error)?;
                            if n == 0 {
                                return Err(Cause::Fat(FatCause::WriteZero));
                            }
                            progress.record.partial_effect.written_bytes += n;
                        }
                        progress.completed();
                        enter(progress, &state, Stage::FileSync, Phase::FileSync, true);
                        #[cfg(any(test, feature = "boot-test"))]
                        if failure == Some(Stage::FileSync) {
                            state.inject_write_failure();
                        }
                        file.flush().map_err(error)?;
                        state.check().map_err(Cause::Stream)?;
                        progress.completed();
                        Ok(())
                    })();
                    if write_result.is_err() {
                        state.fail(stream::Cause::Aborted, 0);
                    }
                    drop(file);
                    write_result?;
                    state.check().map_err(Cause::Stream)?;
                    Data::Created
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
                        state.inject_write_failure();
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
            if writing {
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
                (&mut stream).flush().map_err(Cause::Stream)?;
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
