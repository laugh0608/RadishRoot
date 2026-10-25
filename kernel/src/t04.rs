use crate::{block_driver::Driver, pci, serial::Serial};
use core::fmt::Write;
use radishroot_kernel::storage::{BlockDevice, Error as BlockError, Kind, Partition, gpt};

#[derive(Debug)]
pub enum Error {
    Block(BlockError),
    Gpt(gpt::Error),
    Contract(&'static str),
}
impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Block(e) => e.kind.code(),
            Self::Gpt(e) => e.code(),
            Self::Contract(code) => code,
        }
    }
}
impl From<BlockError> for Error {
    fn from(e: BlockError) -> Self {
        Self::Block(e)
    }
}
impl From<gpt::Error> for Error {
    fn from(e: gpt::Error) -> Self {
        Self::Gpt(e)
    }
}
fn emit(serial: &mut Serial, args: core::fmt::Arguments<'_>) -> Result<(), Error> {
    serial
        .write_fmt(args)
        .map_err(|_| Error::Contract("SERIAL_UNAVAILABLE"))
}
fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |a, b| {
        (a ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}
fn expected_payload() -> [u8; 4096] {
    let mut bytes = [0xcc; 4096];
    for (i, b) in bytes[..1024].iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(17).wrapping_add(0x53);
    }
    for (i, b) in bytes[1023..2023].iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(29).wrapping_add(7);
    }
    bytes
}
fn exercise(serial: &mut Serial, disk: &mut Driver, mode: &str) -> Result<(), Error> {
    let info = disk.info();
    emit(
        serial,
        format_args!(
            "RR:T04:DEVICE sectors={} block_size=512 readonly={} flush={} queue=16\n",
            info.sectors,
            u8::from(info.readonly),
            u8::from(info.flush)
        ),
    )?;
    let layout = gpt::inspect(disk)?;
    let (data_start, data_sectors) = layout.data();
    let (esp_start, esp_sectors) = layout.esp();
    emit(
        serial,
        format_args!(
            "RR:T04:GPT data_start={data_start} data_sectors={data_sectors} esp_start={esp_start} esp_sectors={esp_sectors} degraded={} readonly={} disk_guid={} data_guid={}\n",
            u8::from(layout.degraded()),
            u8::from(layout.readonly() || info.readonly),
            layout.disk_guid,
            layout.data_guid
        ),
    )?;
    if layout.degraded() {
        emit(
            serial,
            format_args!(
                "RR:T04:GPT_DIAGNOSTIC primary={:?} backup={:?}\n",
                layout.primary_issue, layout.backup_issue
            ),
        )?;
    }
    if mode == "t04-primary-gpt" && (!layout.degraded() || layout.primary_issue.is_none()) {
        return Err(Error::Contract("TEST_EXPECTED_GPT_DEGRADED"));
    }
    let before = disk.requests;
    {
        let mut partition = Partition::open(disk, &layout)?;
        let length = partition.byte_len();
        for offset in [length, length - 1, u64::MAX] {
            if partition.write_at(offset, &[0x42; 512]).is_ok() {
                return Err(Error::Contract("TEST_BOUNDARY_ACCEPTED"));
            }
            if partition.read_at(offset, &mut [0; 512]).is_ok() {
                return Err(Error::Contract("TEST_BOUNDARY_ACCEPTED"));
            }
        }
        if partition.readonly() {
            match partition.write_at(0, &[0x42; 512]) {
                Err(e) if e.kind == Kind::ReadOnly => (),
                _ => return Err(Error::Contract("TEST_READ_ONLY_ACCEPTED")),
            }
        }
    }
    if disk.requests != before {
        return Err(Error::Contract("TEST_REJECT_ISSUED_IO"));
    }
    emit(
        serial,
        format_args!(
            "RR:T04:PROTECTION bounds_rejected=6 readonly_rejected={} submitted=0\n",
            u8::from(info.readonly || layout.readonly())
        ),
    )?;
    if mode == "t04-timeout" {
        emit(
            serial,
            format_args!(
                "RR:T04:TIMEOUT_INJECTION mechanism=hide-used-observation submitted=real\n"
            ),
        )?;
        disk.inject_timeout();
        disk.read_blocks(data_start, &mut [0; 512])?;
        return Err(Error::Contract("TEST_TIMEOUT_RETURNED"));
    }
    if matches!(mode, "t04-write" | "t04-reopen") {
        let mut partition = Partition::open(disk, &layout)?;
        let offset = partition
            .byte_len()
            .checked_sub(4096)
            .ok_or(Error::Contract("TEST_VOLUME_TOO_SMALL"))?;
        let mut actual = [0u8; 4096];
        let expected = expected_payload();
        if mode == "t04-write" {
            if partition.readonly() {
                return Err(Error::Contract("TEST_DISK_READ_ONLY"));
            }
            partition.read_at(offset, &mut actual)?;
            if actual != [0xcc; 4096] {
                return Err(Error::Contract("TEST_INITIAL_PATTERN"));
            }
            let mut aligned = [0; 1024];
            for (i, b) in aligned.iter_mut().enumerate() {
                *b = (i as u8).wrapping_mul(17).wrapping_add(0x53);
            }
            partition.write_at(offset, &aligned)?;
            let mut unaligned = [0; 1000];
            for (i, b) in unaligned.iter_mut().enumerate() {
                *b = (i as u8).wrapping_mul(29).wrapping_add(7);
            }
            partition.write_at(offset + 1023, &unaligned)?;
            partition.flush()?;
        } else if !partition.readonly() {
            return Err(Error::Contract("TEST_REOPEN_NOT_READ_ONLY"));
        }
        partition.read_at(offset, &mut actual)?;
        if actual != expected {
            return Err(Error::Contract("BLOCK_READBACK"));
        }
        emit(
            serial,
            format_args!(
                "RR:T04:PAYLOAD mode={mode} offset={offset} length=4096 checksum={:#x} verified=1\n",
                checksum(&actual)
            ),
        )?;
    } else {
        // 默认入口只做真实只读探测，不根据任意磁盘内容发起测试写入。
        let mut first = [0; 512];
        Partition::open(disk, &layout)?.read_at(0, &mut first)?;
        emit(
            serial,
            format_args!("RR:T04:READ length=512 checksum={:#x}\n", checksum(&first)),
        )?;
    }
    if mode == "t04-write" {
        // 已完成写 / flush 后用真实不支持的请求类型检查错误响应，不伪造设备 status。
        disk.inject_unsupported();
        match disk.read_blocks(data_start, &mut [0; 512]) {
            Err(e) if e.kind == Kind::Unsupported => (),
            _ => return Err(Error::Contract("TEST_UNSUPPORTED_RESPONSE")),
        }
        emit(
            serial,
            format_args!("RR:T04:DEVICE_ERROR kind=unsupported source=real-response recovered=1\n"),
        )?;
    }
    Ok(())
}
pub fn run(serial: &mut Serial, device: pci::Device, mode: &str) -> Result<(), Error> {
    emit(serial, format_args!("RR:T04:ENTER\n"))?;
    if mode.starts_with("t04-") {
        emit(serial, format_args!("RR:T04:CASE name={mode}\n"))?;
    }
    let mut disk = Driver::new(device, mode == "t04-no-flush")?;
    let result = exercise(serial, &mut disk, mode);
    if let Err(error) = disk.stop() {
        if let Err(original) = result {
            emit(
                serial,
                format_args!("RR:T04:PRE_STOP_ERROR detail={original:?}\n"),
            )?;
        }
        return Err(error.into());
    }
    emit(
        serial,
        format_args!(
            "RR:T04:STOP reset=confirmed bus_master=0 allocations=0 shared=0 requests={} flushes={}\n",
            disk.requests, disk.flushes
        ),
    )?;
    result?;
    let label = if mode.is_empty() { "probe" } else { mode };
    emit(serial, format_args!("RR:T04:READY mode={label}\n"))
}
