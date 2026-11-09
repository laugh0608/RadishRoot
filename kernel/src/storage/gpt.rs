//! GPT 库负责解码 / CRC；本层负责产品容量、结构关系、分区选择及降级策略。
use super::block::{self, BlockDevice, Info, Kind, Stage};
use core::mem::ManuallyDrop;
use gpt_disk_io::{
    BlockIo, Disk, DiskError,
    gpt_disk_types::{BlockSize, GptHeader, GptHeaderRevision, GptPartitionType, Guid, Lba},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Issue {
    Signature,
    HeaderFormat,
    HeaderCrc,
    Bounds,
    ArrayCrc,
    PartitionRange,
    DuplicateGuid,
    Overlap,
    VolumeSelection,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Io(block::Error),
    ProtectiveMbr,
    Invalid { primary: Issue, backup: Issue },
    Conflict,
}
impl Error {
    pub fn code(self) -> &'static str {
        match self {
            Self::Io(e) => e.kind.code(),
            Self::ProtectiveMbr => "GPT_MBR",
            Self::Invalid { .. } => "GPT_INVALID",
            Self::Conflict => "GPT_CONFLICT",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub(crate) data_start: u64,
    pub(crate) data_sectors: u64,
    pub(crate) esp_start: u64,
    pub(crate) esp_sectors: u64,
    pub(crate) data_readonly: bool,
    pub primary_issue: Option<Issue>,
    pub backup_issue: Option<Issue>,
    pub disk_guid: Guid,
    pub data_guid: Guid,
}
impl Layout {
    pub fn data(&self) -> (u64, u64) {
        (self.data_start, self.data_sectors)
    }
    pub fn esp(&self) -> (u64, u64) {
        (self.esp_start, self.esp_sectors)
    }
    pub fn degraded(&self) -> bool {
        self.primary_issue.is_some() || self.backup_issue.is_some()
    }
    pub fn readonly(&self) -> bool {
        self.data_readonly || self.degraded()
    }
}
struct Reader<'a, D>(&'a mut D);
impl<D: BlockDevice> BlockIo for Reader<'_, D> {
    type Error = block::Error;
    fn block_size(&self) -> BlockSize {
        BlockSize::BS_512
    }
    fn num_blocks(&mut self) -> Result<u64, Self::Error> {
        Ok(self.0.info().sectors)
    }
    fn read_blocks(&mut self, lba: Lba, bytes: &mut [u8]) -> Result<(), Self::Error> {
        self.0.read_blocks(lba.to_u64(), bytes)
    }
    fn write_blocks(&mut self, lba: Lba, _: &[u8]) -> Result<(), Self::Error> {
        Err(block::Error::new(
            Kind::ReadOnly,
            Stage::Write,
            lba.to_u64(),
        ))
    }
    fn flush(&mut self) -> Result<(), Self::Error> {
        self.0.flush()
    }
}
fn library_error(error: DiskError<block::Error>) -> Error {
    match error {
        DiskError::Io(e) => Error::Io(e),
        _ => Error::Io(block::Error::new(Kind::Configuration, Stage::Read, 0)),
    }
}
#[derive(Clone, Copy)]
struct Copy {
    header: GptHeader,
    layout: Layout,
}
enum CopyError {
    Io(Error),
    Invalid(Issue),
}
impl From<Issue> for CopyError {
    fn from(value: Issue) -> Self {
        Self::Invalid(value)
    }
}
fn read_copy<D: BlockDevice>(
    disk: &mut Disk<Reader<'_, D>>,
    info: Info,
    backup: bool,
    storage: &mut [u8; 16384],
) -> Result<Copy, CopyError> {
    let mut block = [0u8; 512];
    let at = if backup { info.sectors - 1 } else { 1 };
    let h = disk
        .read_gpt_header(Lba(at), &mut block)
        .map_err(|e| CopyError::Io(library_error(e)))?;
    if !h.is_signature_valid() {
        return Err(Issue::Signature.into());
    }
    // 库的 CRC 接口覆盖固定 92 字节；拒绝扩展头，不能用部分 CRC 验证更长的声明。
    if h.revision != GptHeaderRevision::VERSION_1_0
        || h.header_size.to_u32() != 92
        || h.reserved.to_u32() != 0
        || block[92..].iter().any(|b| *b != 0)
    {
        return Err(Issue::HeaderFormat.into());
    }
    if h.calculate_header_crc32() != h.header_crc32 {
        return Err(Issue::HeaderCrc.into());
    }
    let disk_guid = h.disk_guid;
    let entries = h.number_of_partition_entries.to_u32();
    let first = h.first_usable_lba.to_u64();
    let last = h.last_usable_lba.to_u64();
    let array_start = h.partition_entry_lba.to_u64();
    let array_blocks = (u64::from(entries) * 128).div_ceil(512);
    let array_end = array_start.checked_add(array_blocks).ok_or(Issue::Bounds)?;
    if h.my_lba.to_u64() != at
        || h.alternate_lba.to_u64() != if backup { 1 } else { info.sectors - 1 }
        || entries == 0
        || entries > 128
        || h.size_of_partition_entry.to_u32() != 128
        || disk_guid == Guid::ZERO
        || first < 34
        || first > last
        || last >= info.sectors - 33
        || array_start < 2
        || array_end > info.sectors - 1
        || (!backup && array_end > first)
        || (backup && array_start <= last)
    {
        return Err(Issue::Bounds.into());
    }
    let location = h
        .get_partition_entry_array_layout()
        .map_err(|_| Issue::Bounds)?;
    let array = disk
        .read_gpt_partition_entry_array(location, storage)
        .map_err(|e| CopyError::Io(library_error(e)))?;
    if array.calculate_crc32() != h.partition_entry_array_crc32 {
        return Err(Issue::ArrayCrc.into());
    }
    let mut ranges = [(0u64, 0u64); 128];
    let mut guids = [Guid::ZERO; 128];
    let mut count = 0;
    let mut data = None;
    let mut esp = None;
    for index in 0..entries {
        let p = array.get_partition_entry(index).ok_or(Issue::Bounds)?;
        if !p.is_used() {
            continue;
        }
        let start = p.starting_lba.to_u64();
        let end = p.ending_lba.to_u64();
        if start < first || end > last || start > end {
            return Err(Issue::PartitionRange.into());
        }
        let unique = p.unique_partition_guid;
        let kind = p.partition_type_guid;
        if unique == Guid::ZERO || guids[..count].contains(&unique) {
            return Err(Issue::DuplicateGuid.into());
        }
        if ranges[..count]
            .iter()
            .any(|(a, b)| start <= *b && *a <= end)
        {
            return Err(Issue::Overlap.into());
        }
        guids[count] = p.unique_partition_guid;
        ranges[count] = (start, end);
        count += 1;
        if kind == GptPartitionType::EFI_SYSTEM {
            if esp.replace((start, end - start + 1)).is_some() {
                return Err(Issue::VolumeSelection.into());
            }
        } else if kind == GptPartitionType::BASIC_DATA {
            if !p.name.chars().eq("DATA".chars())
                || p.attributes.no_block_io_protocol()
                || data.is_some()
            {
                return Err(Issue::VolumeSelection.into());
            }
            data = Some((
                start,
                end - start + 1,
                p.attributes.0.to_u64() & (1 << 60) != 0,
                p.unique_partition_guid,
            ));
        }
    }
    let (data_start, data_sectors, data_readonly, data_guid) =
        data.ok_or(Issue::VolumeSelection)?;
    let (esp_start, esp_sectors) = esp.ok_or(Issue::VolumeSelection)?;
    Ok(Copy {
        header: h,
        layout: Layout {
            data_start,
            data_sectors,
            esp_start,
            esp_sectors,
            data_readonly,
            disk_guid: h.disk_guid,
            data_guid,
            primary_issue: None,
            backup_issue: None,
        },
    })
}
pub fn inspect<D: BlockDevice>(device: &mut D) -> Result<Layout, Error> {
    let info = device.info();
    if info.sectors < 68 || info.sectors > u64::MAX / 512 {
        return Err(Error::Io(block::Error::new(
            Kind::Range,
            Stage::Validate,
            0,
        )));
    }
    let mut mbr = [0; 512];
    device.read_blocks(0, &mut mbr).map_err(Error::Io)?;
    if mbr[510..] != [0x55, 0xaa]
        || mbr[446] != 0
        || mbr[450] != 0xee
        || u32::from_le_bytes(mbr[454..458].try_into().unwrap()) != 1
        || u32::from_le_bytes(mbr[458..462].try_into().unwrap())
            != (info.sectors - 1).min(u32::MAX as u64) as u32
        || mbr[462..510].iter().any(|b| *b != 0)
    {
        return Err(Error::ProtectiveMbr);
    }
    // GPT 仅只读解析；Disk::Drop 会吞 flush 错误，故抑制它。Reader 只持有借用，无自有资源。
    let mut disk = ManuallyDrop::new(Disk::new(Reader(device)).map_err(library_error)?);
    let mut storage = [0u8; 16384];
    let primary = read_copy(&mut disk, info, false, &mut storage);
    if let Err(CopyError::Io(error)) = primary {
        return Err(error);
    }
    let backup = read_copy(&mut disk, info, true, &mut storage);
    if let Err(CopyError::Io(error)) = backup {
        return Err(error);
    }
    match (primary, backup) {
        (Ok(a), Ok(b)) => {
            if a.layout != b.layout
                || a.header.first_usable_lba != b.header.first_usable_lba
                || a.header.last_usable_lba != b.header.last_usable_lba
                || a.header.number_of_partition_entries != b.header.number_of_partition_entries
                || a.header.partition_entry_array_crc32 != b.header.partition_entry_array_crc32
            {
                return Err(Error::Conflict);
            }
            Ok(a.layout)
        }
        (Ok(mut a), Err(CopyError::Invalid(issue))) => {
            a.layout.backup_issue = Some(issue);
            Ok(a.layout)
        }
        (Err(CopyError::Invalid(issue)), Ok(mut b)) => {
            b.layout.primary_issue = Some(issue);
            Ok(b.layout)
        }
        (Err(CopyError::Invalid(primary)), Err(CopyError::Invalid(backup))) => {
            Err(Error::Invalid { primary, backup })
        }
        _ => unreachable!("IO errors returned before copy selection"),
    }
}

#[cfg(test)]
mod tests;
