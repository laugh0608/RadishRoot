use core::fmt;

pub const SECTOR: usize = 512;
pub const MAX_TRANSFER: usize = 32 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Range,
    Alignment,
    ReadOnly,
    FlushUnsupported,
    Configuration,
    Io,
    Unsupported,
    Protocol,
    Timeout,
    PollBudgetExhausted,
    Clock,
    Offline,
}
impl Kind {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Range => "BLOCK_RANGE",
            Self::Alignment => "BLOCK_ALIGNMENT",
            Self::ReadOnly => "BLOCK_READ_ONLY",
            Self::FlushUnsupported => "BLOCK_FLUSH_UNSUPPORTED",
            Self::Configuration => "BLOCK_CONFIG",
            Self::Io => "BLOCK_IO",
            Self::Unsupported => "BLOCK_UNSUPPORTED",
            Self::Protocol => "BLOCK_PROTOCOL",
            Self::Timeout => "BLOCK_TIMEOUT",
            Self::PollBudgetExhausted => "BLOCK_POLL_BUDGET",
            Self::Clock => "BLOCK_CLOCK",
            Self::Offline => "BLOCK_OFFLINE",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Validate,
    Initialize,
    Read,
    Write,
    Flush,
    Complete,
    Reset,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    pub kind: Kind,
    pub stage: Stage,
    pub sector: u64,
    /// 已被设备确认的有效载荷字节；不代表已经持久化。当前失败请求可能有额外未知副作用。
    pub completed_bytes: usize,
}
impl Error {
    pub const fn new(kind: Kind, stage: Stage, sector: u64) -> Self {
        Self {
            kind,
            stage,
            sector,
            completed_bytes: 0,
        }
    }
    fn progress(mut self, completed: usize) -> Self {
        self.completed_bytes += completed;
        self
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} stage={:?} sector={} completed_bytes={}",
            self.kind.code(),
            self.stage,
            self.sector,
            self.completed_bytes
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Info {
    pub sectors: u64,
    pub readonly: bool,
    pub flush: bool,
}
pub fn validate(info: Info, lba: u64, bytes: usize, write: bool) -> Result<(), Error> {
    if bytes == 0 || bytes > MAX_TRANSFER {
        return Err(Error::new(Kind::Range, Stage::Validate, lba));
    }
    if !bytes.is_multiple_of(SECTOR) {
        return Err(Error::new(Kind::Alignment, Stage::Validate, lba));
    }
    if lba
        .checked_add((bytes / SECTOR) as u64)
        .is_none_or(|end| end > info.sectors)
    {
        return Err(Error::new(Kind::Range, Stage::Validate, lba));
    }
    if write && info.readonly {
        return Err(Error::new(Kind::ReadOnly, Stage::Validate, lba));
    }
    if write && !info.flush {
        return Err(Error::new(Kind::FlushUnsupported, Stage::Validate, lba));
    }
    Ok(())
}
/// 仅供存储后端和分区适配使用；UI / 文件服务通过 Partition，不获得整盘写接口。
pub trait BlockDevice {
    fn info(&self) -> Info;
    fn read_blocks(&mut self, lba: u64, bytes: &mut [u8]) -> Result<(), Error>;
    fn write_blocks(&mut self, lba: u64, bytes: &[u8]) -> Result<(), Error>;
    fn flush(&mut self) -> Result<(), Error>;
}
pub struct Partition<'a, D: BlockDevice> {
    device: &'a mut D,
    start: u64,
    bytes: u64,
    readonly: bool,
}
impl<'a, D: BlockDevice> Partition<'a, D> {
    pub fn open(device: &'a mut D, layout: &super::gpt::Layout) -> Result<Self, Error> {
        let info = device.info();
        let (start, sectors) = layout.data();
        let bytes = sectors.checked_mul(SECTOR as u64).ok_or(Error::new(
            Kind::Range,
            Stage::Validate,
            start,
        ))?;
        if sectors == 0
            || start
                .checked_add(sectors)
                .is_none_or(|end| end > info.sectors)
        {
            return Err(Error::new(Kind::Range, Stage::Validate, start));
        }
        Ok(Self {
            device,
            start,
            bytes,
            readonly: info.readonly || layout.readonly(),
        })
    }
    /// 查询事务强制只读，即使底层设备可写也禁止库析构写回。
    pub fn read_only(mut self) -> Self {
        self.readonly = true;
        self
    }
    pub fn byte_len(&self) -> u64 {
        self.bytes
    }
    pub fn readonly(&self) -> bool {
        self.readonly
    }
    fn range(&self, offset: u64, length: usize, write: bool) -> Result<(), Error> {
        if length == 0
            || length > MAX_TRANSFER
            || offset
                .checked_add(length as u64)
                .is_none_or(|end| end > self.bytes)
        {
            return Err(Error::new(Kind::Range, Stage::Validate, self.start));
        }
        if write && self.readonly {
            return Err(Error::new(Kind::ReadOnly, Stage::Validate, self.start));
        }
        if write && !self.device.info().flush {
            return Err(Error::new(
                Kind::FlushUnsupported,
                Stage::Validate,
                self.start,
            ));
        }
        Ok(())
    }
    pub fn read_at(&mut self, offset: u64, output: &mut [u8]) -> Result<(), Error> {
        self.range(offset, output.len(), false)?;
        let mut done = 0;
        let mut sector = [0; SECTOR];
        while done < output.len() {
            let at = offset + done as u64;
            let lba = self.start + at / SECTOR as u64;
            let inside = (at % SECTOR as u64) as usize;
            let n = (SECTOR - inside).min(output.len() - done);
            self.device
                .read_blocks(lba, &mut sector)
                .map_err(|e| e.progress(done))?;
            output[done..done + n].copy_from_slice(&sector[inside..inside + n]);
            done += n;
        }
        Ok(())
    }
    /// 返回只表示设备确认写入；调用者必须检查随后的 flush 才能发布持久化成功。
    pub fn write_at(&mut self, offset: u64, input: &[u8]) -> Result<(), Error> {
        self.range(offset, input.len(), true)?;
        let mut done = 0;
        let mut sector = [0; SECTOR];
        while done < input.len() {
            let at = offset + done as u64;
            let lba = self.start + at / SECTOR as u64;
            let inside = (at % SECTOR as u64) as usize;
            let n = (SECTOR - inside).min(input.len() - done);
            if inside != 0 || n != SECTOR {
                self.device
                    .read_blocks(lba, &mut sector)
                    .map_err(|e| e.progress(done))?;
            }
            sector[inside..inside + n].copy_from_slice(&input[done..done + n]);
            self.device
                .write_blocks(lba, &sector)
                .map_err(|e| e.progress(done))?;
            done += n;
        }
        Ok(())
    }
    pub fn flush(&mut self) -> Result<(), Error> {
        if !self.device.info().flush {
            return Err(Error::new(Kind::FlushUnsupported, Stage::Flush, self.start));
        }
        self.device.flush()
    }
}
