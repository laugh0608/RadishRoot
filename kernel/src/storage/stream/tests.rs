extern crate std;
use super::*;
use crate::storage::{Info, gpt::Layout};
use gpt_disk_io::gpt_disk_types::Guid;
use std::{vec, vec::Vec};

struct Disk {
    bytes: Vec<u8>,
    writes: usize,
    flushes: usize,
    fail_write: bool,
    fail_flush: bool,
    readonly: bool,
}
impl Disk {
    fn new() -> Self {
        Self {
            bytes: vec![0x63; 8 * 512],
            writes: 0,
            flushes: 0,
            fail_write: false,
            fail_flush: false,
            readonly: false,
        }
    }
}
impl BlockDevice for Disk {
    fn info(&self) -> Info {
        Info {
            sectors: 8,
            readonly: self.readonly,
            flush: true,
        }
    }
    fn read_blocks(&mut self, lba: u64, out: &mut [u8]) -> Result<(), block::Error> {
        let start = lba as usize * 512;
        out.copy_from_slice(&self.bytes[start..start + out.len()]);
        Ok(())
    }
    fn write_blocks(&mut self, lba: u64, data: &[u8]) -> Result<(), block::Error> {
        self.writes += 1;
        if self.fail_write {
            return Err(block::Error::new(block::Kind::Io, block::Stage::Write, lba));
        }
        let start = lba as usize * 512;
        self.bytes[start..start + data.len()].copy_from_slice(data);
        Ok(())
    }
    fn flush(&mut self) -> Result<(), block::Error> {
        self.flushes += 1;
        if self.fail_flush {
            return Err(block::Error::new(block::Kind::Io, block::Stage::Flush, 0));
        }
        Ok(())
    }
}
fn layout() -> Layout {
    Layout {
        data_start: 2,
        data_sectors: 4,
        esp_start: 0,
        esp_sectors: 2,
        data_readonly: false,
        primary_issue: None,
        backup_issue: None,
        disk_guid: Guid::from_bytes([1; 16]),
        data_guid: Guid::from_bytes([2; 16]),
    }
}
#[test]
fn crossing_sectors_and_eof_preserve_outside_data() {
    let mut disk = Disk::new();
    let state = State::default();
    {
        let p = Partition::open(&mut disk, &layout()).unwrap();
        let mut stream = Stream::new(p, &state);
        let mut io = &mut stream;
        io.seek(SeekFrom::Start(511)).unwrap();
        io.write_all(&[0xa7; 1025]).unwrap();
        io.flush().unwrap();
        io.seek(SeekFrom::Current(-1025)).unwrap();
        let mut out = [0; 1025];
        io.read_exact(&mut out).unwrap();
        assert_eq!(out, [0xa7; 1025]);
        io.seek(SeekFrom::End(-1)).unwrap();
        let mut tail = [0; 4];
        assert_eq!(io.read(&mut tail).unwrap(), 1);
        assert_eq!(tail, [0x63, 0, 0, 0]);
        assert_eq!(io.read(&mut tail).unwrap(), 0);
        assert_eq!(io.write(&[]).unwrap(), 0);
    }
    let mut expected = vec![0x63; 4096];
    expected[1535..2560].fill(0xa7);
    assert_eq!(disk.bytes, expected);
    assert_eq!(disk.flushes, 1);
}
#[test]
fn invalid_seek_is_sticky_and_never_moves_cursor() {
    for from in [
        SeekFrom::Start(u64::MAX),
        SeekFrom::Current(i64::MIN),
        SeekFrom::End(1),
    ] {
        let mut disk = Disk::new();
        let state = State::default();
        let mut stream = Stream::new(Partition::open(&mut disk, &layout()).unwrap(), &state);
        let error = (&mut stream).seek(from).unwrap_err();
        assert_eq!(error.cause, Cause::Bounds);
        assert_eq!(stream.position(), 0);
        assert_eq!((&mut stream).write(&[1]).unwrap_err(), error);
        assert_eq!(disk.writes, 0);
    }
}
#[test]
fn bounds_failure_does_not_partially_write() {
    let mut disk = Disk::new();
    let state = State::default();
    let mut stream = Stream::new(Partition::open(&mut disk, &layout()).unwrap(), &state);
    (&mut stream).seek(SeekFrom::End(-1)).unwrap();
    assert_eq!(
        (&mut stream).write(&[1; 2]).unwrap_err().cause,
        Cause::Bounds
    );
    assert_eq!(disk.writes, 0);
}
#[test]
fn device_error_retains_original_phase_and_blocks_drop_retries() {
    let mut disk = Disk::new();
    disk.fail_write = true;
    let state = State::default();
    state.phase(Phase::FileSync);
    {
        let mut stream = Stream::new(Partition::open(&mut disk, &layout()).unwrap(), &state);
        let error = (&mut stream).write(&[3]).unwrap_err();
        assert!(matches!(
            error.cause,
            Cause::Block(block::Error {
                stage: block::Stage::Write,
                sector: 2,
                ..
            })
        ));
        state.phase(Phase::Unmount);
        assert_eq!((&mut stream).flush().unwrap_err(), error);
        assert_eq!((&mut stream).write(&[4]).unwrap_err(), error);
        assert_eq!(error.phase, Phase::FileSync);
    }
    assert_eq!(disk.writes, 1);
    assert_eq!(disk.flushes, 0);
    assert_eq!(state.blocked_calls(), 2);
}
#[test]
fn flush_failure_cannot_be_retried_to_success() {
    let mut disk = Disk::new();
    disk.fail_flush = true;
    let state = State::default();
    state.phase(Phase::DeviceFlush);
    {
        let mut stream = Stream::new(Partition::open(&mut disk, &layout()).unwrap(), &state);
        let error = (&mut stream).flush().unwrap_err();
        assert_eq!((&mut stream).flush().unwrap_err(), error);
    }
    assert_eq!(disk.flushes, 1);
}
#[test]
fn readonly_rejection_submits_no_write_or_flush() {
    let mut disk = Disk::new();
    disk.readonly = true;
    let state = State::default();
    {
        let mut stream = Stream::new(Partition::open(&mut disk, &layout()).unwrap(), &state);
        (&mut stream).flush().unwrap();
        assert!(matches!(
            (&mut stream).write(&[1]).unwrap_err().cause,
            Cause::Block(block::Error {
                kind: block::Kind::ReadOnly,
                ..
            })
        ));
    }
    assert_eq!(disk.writes, 0);
    assert_eq!(disk.flushes, 0);
}
