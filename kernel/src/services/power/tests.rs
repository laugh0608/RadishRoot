use super::*;
use crate::{
    services::tests::{Disk, formatted},
    shell::{self, Shell, parser},
    storage::block::{self, BlockDevice, Quiesce},
};
use alloc::{format, vec};
struct Device {
    disk: Disk,
    readonly: bool,
    stopped: bool,
    fail_stop: bool,
    log: Vec<&'static str>,
}
impl BlockDevice for Device {
    fn info(&self) -> block::Info {
        block::Info {
            readonly: self.readonly,
            ..self.disk.info()
        }
    }
    fn read_blocks(&mut self, lba: u64, b: &mut [u8]) -> Result<(), block::Error> {
        assert!(!self.stopped);
        self.disk.read_blocks(lba, b)
    }
    fn write_blocks(&mut self, lba: u64, b: &[u8]) -> Result<(), block::Error> {
        assert!(!self.stopped && !self.readonly);
        self.log.push("write");
        self.disk.write_blocks(lba, b)
    }
    fn flush(&mut self) -> Result<(), block::Error> {
        assert!(!self.stopped);
        self.log.push("flush");
        self.disk.flush()
    }
}
impl Quiesce for Device {
    fn stop(&mut self) -> Result<(), block::Error> {
        assert_eq!(self.log.last(), Some(&"flush"));
        self.log.push("stop");
        if self.fail_stop {
            return Err(block::Error::new(
                block::Kind::Timeout,
                block::Stage::Reset,
                0,
            ));
        }
        self.stopped = true;
        Ok(())
    }
}
fn device(disk: Disk) -> Device {
    Device {
        disk,
        readonly: false,
        stopped: false,
        fail_stop: false,
        log: vec![],
    }
}
#[test]
fn sync_is_real_flush_without_directory_change_and_leaves_service_writable() {
    let (disk, layout) = formatted();
    let mut disk = device(disk);
    let mut files =
        FileService::new(fat::FatBackend::new(&mut disk, layout), BootId([1; 16])).unwrap();
    let mut shell = Shell::new();
    shell
        .execute("write saved.txt payload", &mut files)
        .unwrap()
        .result
        .unwrap();
    let generation = files.generation();
    let changes = files.changes.iter().count();
    let before = files.backend.device.log.len();
    assert!(matches!(
        shell.execute("sync", &mut files).unwrap().result,
        Ok(shell::Output::Synced)
    ));
    assert!(files.backend.device.log[before..].contains(&"flush"));
    assert_eq!(
        (files.generation(), files.changes.iter().count()),
        (generation, changes)
    );
    assert_eq!(files.power(), State::Running);
    let record = files.operations.iter().last().unwrap();
    assert_eq!(
        (record.request_kind, record.status),
        (RequestKind::Sync, Status::Succeeded)
    );
    let stages: Vec<_> = files
        .events
        .iter()
        .filter(|e| e.operation_id == record.id)
        .map(|e| e.stage)
        .collect();
    assert_eq!(
        stages,
        vec![
            Stage::Validate,
            Stage::Validate,
            Stage::Drain,
            Stage::Mount,
            Stage::VolumeSync,
            Stage::Unmount,
            Stage::DeviceFlush,
            Stage::Complete
        ]
    );
    shell
        .execute("write after.txt yes", &mut files)
        .unwrap()
        .result
        .unwrap();
    for name in ["sync", "reboot", "shutdown"] {
        assert!(parser::parse(name).is_ok());
        assert_eq!(
            parser::parse(&format!("{name} now")),
            Err(parser::ParseError::Arguments)
        );
    }
}
#[test]
fn power_prepare_stop_and_request_preserve_identity_and_never_claim_completed_power() {
    let (disk, layout) = formatted();
    let mut disk = device(disk);
    let mut files =
        FileService::new(fat::FatBackend::new(&mut disk, layout), BootId([1; 16])).unwrap();
    let mut shell = Shell::new();
    let reply = shell.execute("shutdown", &mut files).unwrap();
    let id = reply.operation_id.unwrap();
    assert!(matches!(
        reply.result,
        Ok(shell::Output::PowerPrepared(Action::Shutdown))
    ));
    files.power_requested();
    assert_eq!(files.power(), State::Prepared(Action::Shutdown, id));
    for _ in 0..40 {
        shell.execute("ls", &mut files).unwrap().result.unwrap();
    }
    assert!(files.operations.iter().any(|r| r.id == id));
    let before = files.backend.device.log.len();
    let rejected = shell.execute("write late.txt x", &mut files).unwrap();
    assert!(matches!(
        rejected.result,
        Err(shell::Error::Service(SystemError {
            code: Code::ShuttingDown,
            ..
        }))
    ));
    assert_eq!(files.backend.device.log.len(), before);
    files.quiesce_for_power().unwrap();
    assert_eq!(files.power(), State::Stopped(Action::Shutdown, id));
    assert!(files.backend.device.stopped);
    files.power_requested();
    assert_eq!(files.power(), State::Requested(Action::Shutdown, id));
    let record = files.operations.iter().find(|r| r.id == id).unwrap();
    assert_eq!(record.status, Status::Requested);
    files.fail_power(Stage::Platform, Cause::Platform(PlatformCause::NotTaken));
    let record = files.operations.iter().find(|r| r.id == id).unwrap();
    assert_eq!(record.status, Status::Failed);
    assert_eq!(record.error.as_ref().unwrap().code, Code::PowerNotTaken);
    let explain = format!(
        "explain {:032x}:{}",
        u128::from_be_bytes(id.boot.0),
        id.sequence
    );
    assert!(
        matches!(shell.execute(&explain,&mut files).unwrap().result,Ok(shell::Output::Explanation{record,..}) if record.id==id && record.status==Status::Failed)
    );
    assert!(shell.execute("help", &mut files).unwrap().result.is_ok());
}
#[test]
fn sync_failures_keep_stage_stop_writes_and_do_not_prepare_power() {
    for stage in [Stage::VolumeSync, Stage::DeviceFlush] {
        let (disk, layout) = formatted();
        let mut disk = device(disk);
        let mut backend = fat::FatBackend::new(&mut disk, layout);
        backend.inject_sync(stage);
        let mut files = FileService::new(backend, BootId([1; 16])).unwrap();
        let mut shell = Shell::new();
        shell
            .execute("write saved.txt payload", &mut files)
            .unwrap()
            .result
            .unwrap();
        let reply = shell.execute("reboot", &mut files).unwrap();
        let id = reply.operation_id.unwrap();
        assert!(
            matches!(reply.result,Err(shell::Error::Service(SystemError{stage:s,..})) if s==stage)
        );
        assert_eq!(files.power(), State::Failed(Action::Reboot, id));
        assert!(files.prepared_power().is_none());
        assert!(!files.backend.device.log.contains(&"stop"));
        let before = files.backend.device.log.len();
        assert!(
            shell
                .execute("write late.txt x", &mut files)
                .unwrap()
                .result
                .is_err()
        );
        assert_eq!(files.backend.device.log.len(), before);
        assert_eq!(files.volume().mount_state, MountState::Offline);
    }
}
#[test]
fn readonly_sync_still_flushes_device_without_writing_and_stop_failure_blocks_platform() {
    let (disk, layout) = formatted();
    let mut disk = device(disk);
    disk.readonly = true;
    disk.fail_stop = true;
    let mut files =
        FileService::new(fat::FatBackend::new(&mut disk, layout), BootId([1; 16])).unwrap();
    let mut shell = Shell::new();
    assert!(matches!(
        shell.execute("sync", &mut files).unwrap().result,
        Ok(shell::Output::Synced)
    ));
    assert_eq!(files.backend.device.log, vec!["flush"]);
    shell
        .execute("shutdown", &mut files)
        .unwrap()
        .result
        .unwrap();
    assert!(files.quiesce_for_power().is_err());
    files.power_requested();
    assert!(matches!(files.power(), State::Failed(Action::Shutdown, _)));
    let record = files.operations.iter().last().unwrap();
    assert_eq!(record.stage, Stage::DeviceStop);
    assert!(matches!(
        record.error.as_ref().unwrap().cause,
        Cause::Block(block::Error {
            kind: block::Kind::Timeout,
            ..
        })
    ));
}
