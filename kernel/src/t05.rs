//! 真正的 FAT 可行性入口。写入仅由 boot-test 白名单触发，普通启动不创建文件。
extern crate alloc;
use crate::{block_driver::Driver, pci, serial::Serial};
use alloc::{string::String, vec::Vec};
use core::{cell::Cell, fmt::Write as _};
use fatfs::{FileSystem, FsOptions, Read, ReadWriteSeek, Write};
use radishroot_kernel::storage::{
    BlockDevice, Partition, block,
    fat::{self, Directory},
    gpt,
    stream::{self, Cause, Phase, State, Stream},
};

type FatError = fatfs::Error<stream::Error>;
#[derive(Debug)]
pub enum Error {
    Block(block::Error),
    Gpt(gpt::Error),
    Fat(FatError),
    Stream(stream::Error),
    Contract(&'static str),
}
impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Block(e) => e.kind.code(),
            Self::Gpt(e) => e.code(),
            Self::Stream(e) | Self::Fat(fatfs::Error::Io(e)) => match e.cause {
                Cause::Block(e) => e.kind.code(),
                Cause::InjectedWrite => "FAT_WRITEBACK_INJECTED",
                Cause::Bounds => "FAT_BOUNDS",
                Cause::Budget => "FAT_IO_BUDGET",
                _ => "FAT_IO",
            },
            Self::Fat(fatfs::Error::CorruptedFileSystem) => "FAT_CORRUPT",
            Self::Fat(fatfs::Error::NotEnoughSpace) => "FAT_NO_SPACE",
            Self::Fat(_) => "FAT_ERROR",
            Self::Contract(code) => code,
        }
    }
}
impl From<block::Error> for Error {
    fn from(e: block::Error) -> Self {
        Self::Block(e)
    }
}
impl From<gpt::Error> for Error {
    fn from(e: gpt::Error) -> Self {
        Self::Gpt(e)
    }
}
impl From<FatError> for Error {
    fn from(e: FatError) -> Self {
        Self::Fat(e)
    }
}
impl From<stream::Error> for Error {
    fn from(e: stream::Error) -> Self {
        Self::Stream(e)
    }
}
fn emit(serial: &mut Serial, args: core::fmt::Arguments<'_>) -> Result<(), Error> {
    serial
        .write_fmt(args)
        .map_err(|_| Error::Contract("SERIAL_UNAVAILABLE"))
}
const SEED: &[u8] = b"RadishRoot T05 seed v1\n";
const UTF8: &str = "萝卜 UTF-8 persistence\n";
const MOVED: &[u8] = b"same-volume rename, exact bytes\n";
fn write_file<IO: ReadWriteSeek<Error = stream::Error>>(
    root: &Directory<'_, IO>,
    name: &str,
    bytes: &[u8],
    state: &State,
    written: &Cell<usize>,
    inject: bool,
) -> Result<(), Error> {
    state.phase(Phase::Create);
    let mut file = fat::create_new(root, name)?;
    state.phase(Phase::Write);
    let mut at = 0;
    while at < bytes.len() {
        let n = file.write(&bytes[at..])?;
        if n == 0 {
            state.fail(Cause::WriteZero, at as u64);
            return Err(Error::Contract("FAT_WRITE_ZERO"));
        }
        at += n;
        written.set(written.get() + n);
    }
    state.phase(Phase::FileSync);
    #[cfg(feature = "boot-test")]
    if inject {
        state.inject_write_failure();
    }
    #[cfg(not(feature = "boot-test"))]
    let _ = inject;
    file.flush()?;
    drop(file);
    state.check()?;
    Ok(())
}
fn read_file<IO: ReadWriteSeek<Error = stream::Error>>(
    root: &Directory<'_, IO>,
    name: &str,
    expected: &[u8],
    serial: &mut Serial,
    id: &str,
) -> Result<(), Error> {
    fat::validate_path(name).map_err(|_| Error::Contract("FAT_NAME"))?;
    let mut file = root.open_file(name)?;
    let mut buffer = [0u8; 512];
    let mut at = 0;
    let mut hash = 0xcbf29ce484222325u64;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        if at + n > expected.len() || buffer[..n] != expected[at..at + n] {
            return Err(Error::Contract("FAT_CONTENT"));
        }
        for byte in &buffer[..n] {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
        }
        at += n;
    }
    if at != expected.len() {
        return Err(Error::Contract("FAT_LENGTH"));
    }
    emit(
        serial,
        format_args!("RR:T05:FILE id={id} length={at} checksum={hash:#x} verified=1\n"),
    )
}
fn collision<IO: ReadWriteSeek<Error = stream::Error>>(
    root: &Directory<'_, IO>,
    name: &str,
) -> Result<(), Error> {
    match fat::create_new(root, name) {
        Err(fatfs::Error::AlreadyExists) => Ok(()),
        Err(error) => Err(error.into()),
        Ok(_) => Err(Error::Contract("FAT_COLLISION_ACCEPTED")),
    }
}
fn payloads() -> (Vec<u8>, String) {
    let cross = (0..4099).map(|i| ((i * 29 + 7) % 251) as u8).collect();
    (cross, UTF8.repeat(97))
}
fn large_file<IO: ReadWriteSeek<Error = stream::Error>>(
    root: &Directory<'_, IO>,
    state: &State,
    written: &Cell<usize>,
    serial: &mut Serial,
    create: bool,
) -> Result<(), Error> {
    state.phase(if create { Phase::Create } else { Phase::Read });
    let mut file = if create {
        fat::create_new(root, "one-mib.bin")?
    } else {
        root.open_file("one-mib.bin")?
    };
    let mut expected = [0u8; 512];
    for (i, b) in expected.iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(17).wrapping_add(83);
    }
    let mut actual = [0u8; 512];
    for _ in 0..2048 {
        if create {
            state.phase(Phase::Write);
            let mut at = 0;
            while at < expected.len() {
                let n = file.write(&expected[at..])?;
                if n == 0 {
                    state.fail(Cause::WriteZero, 0);
                    return Err(Error::Contract("FAT_WRITE_ZERO"));
                }
                at += n;
                written.set(written.get() + n);
            }
        } else {
            file.read_exact(&mut actual)?;
            if actual != expected {
                return Err(Error::Contract("FAT_CONTENT"));
            }
        }
    }
    if create {
        state.phase(Phase::FileSync);
        file.flush()?;
    } else {
        if file.read(&mut actual)? != 0 {
            return Err(Error::Contract("FAT_LENGTH"));
        }
        emit(
            serial,
            format_args!("RR:T05:FILE id=one-mib length=1048576 verified=1\n"),
        )?;
    }
    drop(file);
    state.check()?;
    Ok(())
}
fn files<IO: ReadWriteSeek<Error = stream::Error>>(
    fs: &FileSystem<IO>,
    state: &State,
    written: &Cell<usize>,
    serial: &mut Serial,
    mode: &str,
) -> Result<(), Error> {
    let root = fs.root_dir();
    state.phase(Phase::Read);
    read_file(&root, "seed.txt", SEED, serial, "seed")?;
    if mode == "t05-readonly" {
        write_file(
            &root,
            "must-not-exist.txt",
            b"forbidden",
            state,
            written,
            false,
        )?;
        return Err(Error::Contract("FAT_READ_ONLY_ACCEPTED"));
    }
    if mode == "t05-file-sync" {
        emit(
            serial,
            format_args!("RR:T05:INJECTION stage=FileSync mechanism=adapter-next-write\n"),
        )?;
        write_file(&root, "failed.txt", &[0xa5; 1649], state, written, true)?;
        return Err(Error::Contract("FAT_INJECTION_MISSED"));
    }
    let (cross, text) = payloads();
    let long_ascii = "a".repeat(255);
    let long_utf8 = "卜".repeat(85);
    if mode == "t05-write" || mode == "t05-device-flush" || mode == "t05-volume-sync" {
        write_file(&root, "cross-sector.bin", &cross, state, written, false)?;
        write_file(
            &root,
            "萝卜 文件.txt",
            text.as_bytes(),
            state,
            written,
            false,
        )?;
        large_file(&root, state, written, serial, true)?;
        for (name, bytes) in [
            ("ReadMe.txt", b"case-preserved\n".as_slice()),
            ("Straße.txt", b"unicode case\n"),
            ("Long File Name.txt", b"short alias\n"),
            ("é.txt", b"composed\n"),
            ("e\u{301}.txt", b"decomposed\n"),
            (long_ascii.as_str(), b"255 bytes\n"),
            (long_utf8.as_str(), b"85 BMP characters\n"),
        ] {
            write_file(&root, name, bytes, state, written, false)?;
        }
        for name in ["readme.TXT", "STRASSE.TXT", "LONGFI~1.TXT", "seed.txt"] {
            collision(&root, name)?;
        }
        for invalid in ["../escape", "bad.", "bad ", "bad:name", "🦀.txt", "a//b"] {
            if fat::create_new(&root, invalid).is_ok() {
                return Err(Error::Contract("FAT_INVALID_NAME_ACCEPTED"));
            }
        }
        for invalid in ["a".repeat(256), "卜".repeat(86)] {
            if fat::create_new(&root, &invalid).is_ok() {
                return Err(Error::Contract("FAT_LONG_NAME_ACCEPTED"));
            }
        }
        state.phase(Phase::Create);
        let folder = root.create_dir("folder")?;
        collision(&root, "folder")?;
        write_file(&root, "move.txt", MOVED, state, written, false)?;
        root.rename("move.txt", &root, "renamed.txt")?;
        root.rename("renamed.txt", &folder, "moved.txt")?;
        // 锁定库对仅大小写变化返回成功但不改名，能力记录为 no-op，不宣称已更名。
        root.rename("ReadMe.txt", &root, "README.TXT")?;
        if !root
            .iter()
            .any(|e| e.is_ok_and(|e| e.file_name() == "ReadMe.txt"))
        {
            return Err(Error::Contract("FAT_CASE_RENAME_SEMANTICS"));
        }
        drop(folder);
        emit(
            serial,
            format_args!(
                "RR:T05:NAMES collisions=5 invalid=8 same_dir_rename=1 cross_dir_file_rename=1 case_only_rename=no-op normalization=none\n"
            ),
        )?;
    }
    state.phase(Phase::Read);
    for (name, expected, id) in [
        ("cross-sector.bin", cross.as_slice(), "cross"),
        ("萝卜 文件.txt", text.as_bytes(), "utf8"),
        ("folder/moved.txt", MOVED, "moved"),
        ("ReadMe.txt", b"case-preserved\n", "case"),
        ("Straße.txt", b"unicode case\n", "unicode-case"),
        ("Long File Name.txt", b"short alias\n", "alias"),
        ("é.txt", b"composed\n", "composed"),
        ("e\u{301}.txt", b"decomposed\n", "decomposed"),
        (long_ascii.as_str(), b"255 bytes\n", "ascii-limit"),
        (long_utf8.as_str(), b"85 BMP characters\n", "utf8-limit"),
    ] {
        read_file(&root, name, expected, serial, id)?;
    }
    large_file(&root, state, written, serial, false)?;
    drop(root);
    state.check()?;
    Ok(())
}
fn exercise(
    serial: &mut Serial,
    disk: &mut Driver,
    mode: &str,
    state: &State,
    written: &Cell<usize>,
) -> Result<(), Error> {
    let layout = gpt::inspect(disk)?;
    let partition = Partition::open(disk, &layout)?;
    let mut stream = Stream::new(partition, state);
    let readonly = stream.readonly();
    emit(
        serial,
        format_args!(
            "RR:T05:VOLUME readonly={} degraded={} data_start={} data_sectors={}\n",
            u8::from(readonly),
            u8::from(layout.degraded()),
            layout.data().0,
            layout.data().1
        ),
    )?;
    let fs = FileSystem::new(&mut stream, FsOptions::new())?;
    if fs.fat_type() != fatfs::FatType::Fat32 {
        return Err(Error::Contract("FAT_NOT_FAT32"));
    }
    let result = files(&fs, state, written, serial, mode);
    if let Err(error) = result {
        // FAT 语义错误也必须禁止后续 Drop 写入；原始错误返回，不把 Aborted 当作原因。
        state.fail(Cause::Aborted, 0);
        drop(fs);
        return Err(error);
    }
    state.phase(Phase::VolumeSync);
    #[cfg(feature = "boot-test")]
    if mode == "t05-volume-sync" {
        emit(
            serial,
            format_args!("RR:T05:INJECTION stage=VolumeSync mechanism=adapter-next-write\n"),
        )?;
        state.inject_write_failure();
    }
    fs.flush()?;
    state.check()?;
    emit(
        serial,
        format_args!("RR:T05:SYNC stage=VolumeSync result=ok\n"),
    )?;
    state.phase(Phase::Unmount);
    fs.unmount()?;
    state.check()?;
    emit(
        serial,
        format_args!("RR:T05:SYNC stage=Unmount result=ok\n"),
    )?;
    // FileSystem 已销毁，最终 flush 必须位于全部 metadata / dirty flag 写入之后。
    state.phase(Phase::DeviceFlush);
    // 先结束分区借用，复用真实 Driver 的 unsupported 响应注入，错误仍标记为 Flush。
    if mode == "t05-device-flush" {
        emit(
            serial,
            format_args!(
                "RR:T05:INJECTION stage=DeviceFlush mechanism=unsupported-request response=real\n"
            ),
        )?;
        disk.inject_unsupported();
        disk.flush().map_err(|e| state.fail(Cause::Block(e), 0))?;
        return Err(Error::Contract("FAT_INJECTION_MISSED"));
    }
    (&mut stream).flush()?;
    state.check()?;
    emit(
        serial,
        format_args!(
            "RR:T05:SYNC stage=DeviceFlush result=ok readonly={}\n",
            u8::from(readonly)
        ),
    )?;
    Ok(())
}
pub fn run(serial: &mut Serial, device: pci::Device, mode: &str) -> Result<(), Error> {
    emit(
        serial,
        format_args!("RR:T05:ENTER\nRR:T05:CASE name={mode}\n"),
    )?;
    let mut disk = Driver::new(device, false)?;
    let state = State::default();
    let written = Cell::new(0);
    let result = exercise(serial, &mut disk, mode, &state, &written);
    emit(
        serial,
        format_args!(
            "RR:T05:IO calls={} blocked={} written={} first_error={} persistence={}\n",
            state.calls(),
            state.blocked_calls(),
            written.get(),
            u8::from(state.first_error().is_some()),
            if result.is_ok() {
                "confirmed"
            } else {
                "unconfirmed"
            }
        ),
    )?;
    if let Some(error) = state.first_error() {
        emit(
            serial,
            format_args!("RR:T05:FIRST_ERROR detail={error:?}\n"),
        )?;
    }
    if let Err(error) = disk.stop() {
        if let Err(original) = result {
            emit(
                serial,
                format_args!("RR:T05:PRE_STOP_ERROR detail={original:?}\n"),
            )?;
        }
        return Err(error.into());
    }
    emit(
        serial,
        format_args!(
            "RR:T05:STOP reset=confirmed bus_master=0 allocations=0 shared=0 requests={} flushes={}\n",
            disk.requests, disk.flushes
        ),
    )?;
    result?;
    emit(serial, format_args!("RR:T05:READY mode={mode}\n"))
}
