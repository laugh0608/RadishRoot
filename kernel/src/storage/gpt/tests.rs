extern crate std;
use super::*;
use crate::storage::{
    Partition,
    block::{SECTOR, validate},
};
use std::{vec, vec::Vec};

struct MemoryDisk {
    bytes: Vec<u8>,
    readonly: bool,
    flush_supported: bool,
    reads: usize,
    writes: usize,
    flushes: usize,
    fail_read: Option<usize>,
    fail_write: Option<usize>,
    fail_flush: bool,
}
impl BlockDevice for MemoryDisk {
    fn info(&self) -> Info {
        Info {
            sectors: (self.bytes.len() / 512) as u64,
            readonly: self.readonly,
            flush: self.flush_supported,
        }
    }
    fn read_blocks(&mut self, lba: u64, out: &mut [u8]) -> Result<(), block::Error> {
        validate(self.info(), lba, out.len(), false)?;
        self.reads += 1;
        if self.fail_read == Some(self.reads) {
            return Err(block::Error::new(Kind::Io, Stage::Read, lba));
        }
        let start = lba as usize * SECTOR;
        out.copy_from_slice(&self.bytes[start..start + out.len()]);
        Ok(())
    }
    fn write_blocks(&mut self, lba: u64, input: &[u8]) -> Result<(), block::Error> {
        validate(self.info(), lba, input.len(), true)?;
        self.writes += 1;
        if self.fail_write == Some(self.writes) {
            return Err(block::Error::new(Kind::Io, Stage::Write, lba));
        }
        let start = lba as usize * SECTOR;
        self.bytes[start..start + input.len()].copy_from_slice(input);
        Ok(())
    }
    fn flush(&mut self) -> Result<(), block::Error> {
        self.flushes += 1;
        if self.fail_flush {
            Err(block::Error::new(Kind::Io, Stage::Flush, 0))
        } else {
            Ok(())
        }
    }
}
// fixture 独立编码，不调用 gpt_disk_io / types 的生成、CRC 或生产校验函数。
fn crc(bytes: &[u8]) -> u32 {
    let mut result = !0u32;
    for byte in bytes {
        result ^= *byte as u32;
        for _ in 0..8 {
            result = (result >> 1) ^ (0xedb88320 & (0u32.wrapping_sub(result & 1)));
        }
    }
    !result
}
fn put32(bytes: &mut [u8], at: usize, n: u32) {
    bytes[at..at + 4].copy_from_slice(&n.to_le_bytes());
}
fn put64(bytes: &mut [u8], at: usize, n: u64) {
    bytes[at..at + 8].copy_from_slice(&n.to_le_bytes());
}
fn header_crc(bytes: &mut [u8], sector: usize) {
    let h = &mut bytes[sector * 512..sector * 512 + 92];
    put32(h, 16, 0);
    let checksum = crc(h);
    put32(h, 16, checksum);
}
fn arrays(bytes: &mut [u8]) {
    let entry = bytes[1024..1024 + 16384].to_vec();
    bytes[479 * 512..511 * 512].copy_from_slice(&entry);
    for sector in [1, 511] {
        put32(bytes, sector * 512 + 88, crc(&entry));
        header_crc(bytes, sector);
    }
}
fn fixture() -> MemoryDisk {
    let mut b = vec![0u8; 512 * 512];
    b[510..512].copy_from_slice(&[0x55, 0xaa]);
    b[450] = 0xee;
    put32(&mut b, 454, 1);
    put32(&mut b, 458, 511);
    for (sector, other, array) in [(1, 511, 2), (511, 1, 479)] {
        let h = &mut b[sector * 512..sector * 512 + 92];
        h[..8].copy_from_slice(b"EFI PART");
        put32(h, 8, 0x10000);
        put32(h, 12, 92);
        put64(h, 24, sector as u64);
        put64(h, 32, other);
        put64(h, 40, 34);
        put64(h, 48, 478);
        h[56..72].fill(0x11);
        put64(h, 72, array);
        put32(h, 80, 128);
        put32(h, 84, 128);
    }
    let esp = [
        0x28, 0x73, 0x2a, 0xc1, 0x1f, 0xf8, 0xd2, 0x11, 0xba, 0x4b, 0, 0xa0, 0xc9, 0x3e, 0xc9, 0x3b,
    ];
    let data = [
        0xa2, 0xa0, 0xd0, 0xeb, 0xe5, 0xb9, 0x33, 0x44, 0x87, 0xc0, 0x68, 0xb6, 0xb7, 0x26, 0x99,
        0xc7,
    ];
    for (index, kind, start, end, name) in [(0, esp, 64, 127, "ESP"), (1, data, 128, 478, "DATA")] {
        let p = &mut b[1024 + index * 128..1024 + (index + 1) * 128];
        p[..16].copy_from_slice(&kind);
        p[16..32].fill(index as u8 + 1);
        put64(p, 32, start);
        put64(p, 40, end);
        for (i, c) in name.encode_utf16().enumerate() {
            p[56 + i * 2..58 + i * 2].copy_from_slice(&c.to_le_bytes());
        }
    }
    arrays(&mut b);
    MemoryDisk {
        bytes: b,
        readonly: false,
        flush_supported: true,
        reads: 0,
        writes: 0,
        flushes: 0,
        fail_read: None,
        fail_write: None,
        fail_flush: false,
    }
}
#[test]
fn independent_fixture_and_no_hidden_flush() {
    assert_eq!(crc(b"123456789"), 0xcbf43926);
    let mut disk = fixture();
    let layout = inspect(&mut disk).unwrap();
    assert_eq!(layout.data(), (128, 351));
    assert_eq!(layout.esp(), (64, 64));
    assert!(!layout.readonly());
    assert_eq!((disk.reads, disk.writes, disk.flushes), (5, 0, 0));
}
#[test]
fn invalid_primary_is_explicit_readonly_and_both_bad_refuse() {
    let mut disk = fixture();
    disk.bytes[512 + 16] ^= 1;
    let layout = inspect(&mut disk).unwrap();
    assert_eq!(layout.primary_issue, Some(Issue::HeaderCrc));
    let mut p = Partition::open(&mut disk, &layout).unwrap();
    assert_eq!(p.write_at(0, &[1; 512]).unwrap_err().kind, Kind::ReadOnly);
    assert_eq!(disk.writes, 0);
    disk.bytes[511 * 512] ^= 1;
    assert_eq!(
        inspect(&mut disk),
        Err(Error::Invalid {
            primary: Issue::HeaderCrc,
            backup: Issue::Signature
        })
    );
    let mut disk = fixture();
    disk.bytes[479 * 512] ^= 1;
    assert_eq!(
        inspect(&mut disk).unwrap().backup_issue,
        Some(Issue::ArrayCrc)
    );
}
#[test]
fn valid_but_different_copies_are_not_selected_silently() {
    let mut disk = fixture();
    disk.bytes[479 * 512 + 128 + 48 + 7] |= 0x10;
    let checksum = crc(&disk.bytes[479 * 512..511 * 512]);
    put32(&mut disk.bytes, 511 * 512 + 88, checksum);
    header_crc(&mut disk.bytes, 511);
    assert_eq!(inspect(&mut disk), Err(Error::Conflict));
}
#[test]
fn rejects_overlap_duplicate_guid_and_outside_volume() {
    for (offset, value, issue) in [(32, 100, Issue::Overlap), (40, 479, Issue::PartitionRange)] {
        let mut disk = fixture();
        put64(&mut disk.bytes, 1024 + 128 + offset, value);
        arrays(&mut disk.bytes);
        assert_eq!(
            inspect(&mut disk),
            Err(Error::Invalid {
                primary: issue,
                backup: issue
            })
        );
    }
    let mut disk = fixture();
    disk.bytes[1024 + 128 + 16..1024 + 128 + 32].fill(1);
    arrays(&mut disk.bytes);
    assert_eq!(
        inspect(&mut disk),
        Err(Error::Invalid {
            primary: Issue::DuplicateGuid,
            backup: Issue::DuplicateGuid
        })
    );
}
#[test]
fn malformed_counts_do_not_drive_unbounded_io() {
    let mut disk = fixture();
    for sector in [1, 511] {
        put32(&mut disk.bytes, sector * 512 + 80, u32::MAX);
        header_crc(&mut disk.bytes, sector);
    }
    assert_eq!(
        inspect(&mut disk),
        Err(Error::Invalid {
            primary: Issue::Bounds,
            backup: Issue::Bounds
        })
    );
    assert_eq!(disk.reads, 3);
}
#[test]
fn io_failure_is_not_a_format_fallback() {
    let mut disk = fixture();
    disk.fail_read = Some(2);
    assert_eq!(
        inspect(&mut disk),
        Err(Error::Io(block::Error::new(Kind::Io, Stage::Read, 1)))
    );
    assert_eq!(disk.reads, 2);
}
#[test]
fn unaligned_write_preserves_other_bytes_and_flush_failure() {
    let mut disk = fixture();
    let layout = inspect(&mut disk).unwrap();
    disk.bytes[128 * 512..479 * 512].fill(0x63);
    let before = disk.bytes.clone();
    let input = vec![0xa7; 1123];
    {
        let mut p = Partition::open(&mut disk, &layout).unwrap();
        p.write_at(511, &input).unwrap();
        p.flush().unwrap();
        let mut output = vec![0; 1123];
        p.read_at(511, &mut output).unwrap();
        assert_eq!(output, input);
    }
    let mut expected = before;
    expected[128 * 512 + 511..128 * 512 + 511 + 1123].fill(0xa7);
    assert_eq!(disk.bytes, expected);
    disk.fail_flush = true;
    assert_eq!(
        Partition::open(&mut disk, &layout)
            .unwrap()
            .flush()
            .unwrap_err()
            .stage,
        Stage::Flush
    );
}
#[test]
fn whole_range_rejected_before_any_io_and_partial_write_is_reported() {
    let mut disk = fixture();
    let layout = inspect(&mut disk).unwrap();
    let reads = disk.reads;
    {
        let mut p = Partition::open(&mut disk, &layout).unwrap();
        let len = p.byte_len();
        for at in [len, len - 1, u64::MAX] {
            assert_eq!(p.write_at(at, &[3; 512]).unwrap_err().kind, Kind::Range);
        }
    }
    assert_eq!(disk.reads, reads);
    assert_eq!(disk.writes, 0);
    disk.fail_write = Some(2);
    let error = Partition::open(&mut disk, &layout)
        .unwrap()
        .write_at(511, &[3; 1024])
        .unwrap_err();
    assert_eq!(
        (error.stage, error.completed_bytes, error.sector),
        (Stage::Write, 1, 129)
    );
}
#[test]
fn block_validation_and_missing_flush_reject_before_mutation() {
    let mut disk = fixture();
    let layout = inspect(&mut disk).unwrap();
    assert_eq!(
        validate(disk.info(), u64::MAX, 512, false)
            .unwrap_err()
            .kind,
        Kind::Range
    );
    assert_eq!(
        validate(disk.info(), 0, 513, false).unwrap_err().kind,
        Kind::Alignment
    );
    assert_eq!(
        validate(disk.info(), 0, 0, false).unwrap_err().kind,
        Kind::Range
    );
    disk.flush_supported = false;
    assert_eq!(
        Partition::open(&mut disk, &layout)
            .unwrap()
            .write_at(0, &[3; 512])
            .unwrap_err()
            .kind,
        Kind::FlushUnsupported
    );
    assert_eq!(disk.writes, 0);
}
