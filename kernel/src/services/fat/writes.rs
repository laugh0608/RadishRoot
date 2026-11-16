//! 在一个 FAT 事务内持有文件句柄，写入出错时先锁存首错，再释放句柄。
use super::*;

const COPY_CHUNK: usize = 4096;

#[derive(Clone, Copy)]
pub(super) enum Payload<'a> {
    Text(&'a str),
    File(&'a Path),
}

fn write_bytes(
    file: &mut impl Write<Error = fatfs::Error<stream::Error>>,
    mut bytes: &[u8],
    effect: &mut PartialEffect,
) -> Result<(), Cause> {
    while !bytes.is_empty() {
        let written = file.write(bytes).map_err(error)?;
        if written == 0 {
            return Err(Cause::Fat(FatCause::WriteZero));
        }
        effect.written_bytes += written;
        bytes = &bytes[written..];
    }
    Ok(())
}

pub(super) fn write_payload<IO: fatfs::ReadWriteSeek<Error = stream::Error>>(
    root: &fat::Directory<'_, IO>,
    path: &Path,
    payload: Payload<'_>,
    overwrite: bool,
    state: &State,
    progress: &mut Progress,
    #[cfg(any(test, feature = "boot-test"))] failure: Option<Stage>,
) -> Result<Data, Cause> {
    let mut source = None;
    let mut target = None;
    let result = (|| {
        if let Payload::File(source_path) = payload {
            enter(progress, state, Stage::Read, Phase::Read, false);
            let file = source.insert(root.open_file(source_path.relative()).map_err(error)?);
            progress.record.total_bytes = Some(file.seek(SeekFrom::End(0)).map_err(error)?);
            file.seek(SeekFrom::Start(0)).map_err(error)?;
            progress.completed();
        }
        let file = if overwrite {
            enter(progress, state, Stage::Truncate, Phase::Truncate, false);
            let file = target.insert(root.open_file(path.relative()).map_err(error)?);
            file.truncate().map_err(error)?;
            progress.record.partial_effect.target_truncated = true;
            file
        } else {
            enter(progress, state, Stage::Create, Phase::Create, false);
            let file = target.insert(fat::create_new(root, path.relative()).map_err(error)?);
            progress.record.partial_effect.created = true;
            file
        };
        progress.completed();

        let output = match payload {
            Payload::Text(text) => {
                enter(progress, state, Stage::Write, Phase::Write, false);
                write_bytes(file, text.as_bytes(), &mut progress.record.partial_effect)?;
                if overwrite {
                    Data::Overwritten
                } else {
                    Data::Created
                }
            }
            Payload::File(_) => {
                enter(progress, state, Stage::Copy, Phase::Read, false);
                let input = source.as_mut().expect("opened copy source");
                let mut remaining = progress.record.total_bytes.expect("copy source length");
                let mut buffer = [0; COPY_CHUNK];
                while remaining != 0 {
                    let length = remaining.min(COPY_CHUNK as u64) as usize;
                    state.phase(Phase::Read);
                    input.read_exact(&mut buffer[..length]).map_err(error)?;
                    state.phase(Phase::Write);
                    #[cfg(any(test, feature = "boot-test"))]
                    if failure == Some(Stage::Copy)
                        && progress.record.partial_effect.written_bytes != 0
                    {
                        state.inject_write_failure();
                    }
                    write_bytes(file, &buffer[..length], &mut progress.record.partial_effect)?;
                    remaining -= length as u64;
                }
                Data::Copied
            }
        };
        progress.completed();
        enter(progress, state, Stage::FileSync, Phase::FileSync, true);
        #[cfg(any(test, feature = "boot-test"))]
        if failure == Some(Stage::FileSync) {
            state.inject_write_failure();
        }
        file.flush().map_err(error)?;
        state.check().map_err(Cause::Stream)?;
        progress.completed();
        Ok(output)
    })();
    let result = result.map_err(|cause| {
        let cause = state.first_error().map_or(cause, Cause::Stream);
        state.fail(stream::Cause::Aborted, 0);
        cause
    });
    drop(target);
    drop(source);
    let output = result?;
    state.check().map_err(Cause::Stream)?;
    Ok(output)
}
