use super::*;
#[test]
fn paths_are_component_bounded() {
    let root = Path::root();
    assert_eq!(
        root.resolve("/datax/a"),
        Err(path::PathError::OutsideVolume)
    );
    assert_eq!(root.resolve("../x"), Err(path::PathError::OutsideVolume));
    assert_eq!(
        root.resolve("/data/a/../萝卜.txt").unwrap().as_str(),
        "/data/萝卜.txt"
    );
    assert_eq!(root.resolve("folder").unwrap().parent(), root);
    for bad in ["", "a//b", "bad:/../ok", "🦀.txt"] {
        assert!(root.resolve(bad).is_err());
    }
}
use crate::storage::{
    block, gpt,
    stream::{State, Stream},
};
use alloc::{collections::BTreeMap, vec};
use gpt_disk_io::gpt_disk_types::Guid;

struct MemoryBackend {
    calls: usize,
    read_only: bool,
    failure: Option<Stage>,
}
impl Backend for MemoryBackend {
    fn volume(&self) -> VolumeInfo {
        VolumeInfo {
            id: VolumeId([2; 16]),
            capacity_bytes: 1_000_000,
            read_only: self.read_only,
            flush_support: true,
            mount_state: MountState::Unchecked,
        }
    }
    fn run(
        &mut self,
        _: &Path,
        request: BackendRequest<'_>,
        p: &mut Progress,
    ) -> Result<Data, Cause> {
        self.calls += 1;
        p.mounted = true;
        match request {
            BackendRequest::List { .. } => Ok(Data::List {
                entries: vec![RawEntry {
                    name: String::from("one.txt"),
                    kind: FileKind::File,
                    size_bytes: Some(7),
                }],
                next: Some(1),
            }),
            BackendRequest::Read { .. } => Ok(Data::Read {
                bytes: b"payload".to_vec(),
                eof: true,
            }),
            BackendRequest::CreateText(text) => {
                p.enter(Stage::Create, Status::Running);
                p.record.partial_effect.created = true;
                p.record.partial_effect.may_have_changed = true;
                p.completed();
                p.enter(Stage::Write, Status::Running);
                p.record.partial_effect.written_bytes = text.len();
                p.completed();
                for stage in [
                    Stage::FileSync,
                    Stage::VolumeSync,
                    Stage::Unmount,
                    Stage::DeviceFlush,
                ] {
                    p.enter(stage, Status::Flushing);
                    if self.failure == Some(stage) {
                        return Err(Cause::Contract(Code::Io));
                    }
                    p.completed();
                }
                p.record.partial_effect.persistence_confirmed = true;
                Ok(Data::Created)
            }
        }
    }
}
fn service() -> FileService<MemoryBackend> {
    FileService::new(
        MemoryBackend {
            calls: 0,
            read_only: false,
            failure: None,
        },
        BootId([1; 16]),
    )
    .unwrap()
}
fn create<'a>(path: &'a str, text: &'a str) -> Request<'a> {
    Request::CreateText { path, text }
}
#[test]
fn validation_and_readonly_are_rejected_before_backend_calls() {
    let mut s = service();
    let root = Path::root();
    for request in [
        create("/datax/escape", "x"),
        create("../escape", "x"),
        Request::Read {
            path: "a",
            offset: u64::MAX,
            limit: 1,
            reference: None,
        },
        Request::Read {
            path: "a",
            offset: 0,
            limit: 0,
            reference: None,
        },
    ] {
        assert!(s.execute(&root, request).unwrap().result.is_err());
    }
    s.volume.read_only = true;
    assert_eq!(
        s.execute(&root, create("valid.txt", "ok"))
            .unwrap()
            .result
            .unwrap_err()
            .code,
        Code::ReadOnly
    );
    assert_eq!(s.backend.calls, 0);
    assert_eq!(s.generation(), 0);
    assert_eq!(s.changes.iter().count(), 0);
}
#[test]
fn references_and_pagination_bind_boot_volume_path_and_generation() {
    let mut s = service();
    let root = Path::root();
    let Output::List(page) = s
        .execute(
            &root,
            Request::List {
                path: ".",
                cursor: None,
            },
        )
        .unwrap()
        .result
        .unwrap()
    else {
        panic!()
    };
    let r = page.entries[0].reference.clone();
    for changed in [
        FileRef {
            boot: BootId([3; 16]),
            ..r.clone()
        },
        FileRef {
            volume: VolumeId([4; 16]),
            ..r.clone()
        },
        FileRef {
            generation: 8,
            ..r.clone()
        },
    ] {
        assert_eq!(
            s.execute(
                &root,
                Request::Read {
                    path: "one.txt",
                    offset: 0,
                    limit: 8,
                    reference: Some(&changed)
                }
            )
            .unwrap()
            .result
            .unwrap_err()
            .code,
            Code::StaleReference
        );
    }
    let reply = s.execute(&root, create("new.txt", "abc")).unwrap();
    assert!(reply.result.is_ok());
    assert_eq!(s.generation(), 1);
    let e = s.changes.iter().last().unwrap();
    assert_eq!(e.operation_id, reply.operation_id);
    assert_eq!(e.paths[1], root);
    assert_eq!(
        s.execute(
            &root,
            Request::List {
                path: ".",
                cursor: page.next.as_ref()
            }
        )
        .unwrap()
        .result
        .unwrap_err()
        .code,
        Code::StaleReference
    );
    assert_eq!(
        s.execute(
            &root,
            Request::Read {
                path: "one.txt",
                offset: 0,
                limit: 8,
                reference: Some(&r)
            }
        )
        .unwrap()
        .result
        .unwrap_err()
        .code,
        Code::StaleReference
    );
    assert_eq!(s.backend.calls, 2);
}
#[test]
fn every_sync_failure_retains_progress_and_never_publishes_success() {
    for stage in [
        Stage::FileSync,
        Stage::VolumeSync,
        Stage::Unmount,
        Stage::DeviceFlush,
    ] {
        let mut s = service();
        s.backend.failure = Some(stage);
        let reply = s
            .execute(&Path::root(), create("partial.txt", "abc"))
            .unwrap();
        let e = reply.result.unwrap_err();
        assert_eq!(e.stage, stage);
        assert_eq!(e.partial_effect.written_bytes, 3);
        assert!(e.partial_effect.created);
        assert!(!e.partial_effect.persistence_confirmed);
        assert_eq!(s.volume().mount_state, MountState::Offline);
        assert_eq!(s.changes.iter().count(), 1);
        assert!(!s.events.iter().any(|e| e.status == Status::Succeeded));
        assert_eq!(s.operations.iter().last().unwrap().error.as_ref(), Some(&e));
        assert_eq!(
            s.execute(&Path::root(), create("later.txt", "x"))
                .unwrap()
                .result
                .unwrap_err()
                .code,
            Code::Offline
        );
        assert_eq!(s.backend.calls, 1);
    }
}
#[test]
fn histories_report_eviction_and_ids_never_wrap() {
    let mut s = service();
    for _ in 0..48 {
        assert!(
            s.execute(
                &Path::root(),
                Request::List {
                    path: ".",
                    cursor: None
                }
            )
            .unwrap()
            .result
            .is_ok()
        );
    }
    assert_eq!(s.operations.iter().count(), 32);
    assert_eq!(s.operations.evicted, 16);
    assert!(s.events.evicted > 0);
    let ids: Vec<_> = s.events.iter().map(|e| e.seq).collect();
    assert!(ids.windows(2).all(|w| w[0] + 1 == w[1]));
    let calls = s.backend.calls;
    s.next_operation = u64::MAX;
    assert!(matches!(
        s.execute(&Path::root(), create("x", "")),
        Err(Code::IdentityExhausted)
    ));
    assert_eq!(s.backend.calls, calls);
    s.next_operation = 1;
    s.next_event = u64::MAX;
    assert!(matches!(
        s.execute(&Path::root(), create("x", "")),
        Err(Code::IdentityExhausted)
    ));
    assert_eq!(s.backend.calls, calls);
    s.next_event = 1;
    s.generation = u64::MAX;
    assert_eq!(
        s.execute(&Path::root(), create("x", ""))
            .unwrap()
            .result
            .unwrap_err()
            .code,
        Code::IdentityExhausted
    );
    assert_eq!(s.backend.calls, calls);
}

struct Disk {
    sectors: BTreeMap<u64, [u8; 512]>,
    writes: usize,
    flushes: usize,
}
impl block::BlockDevice for Disk {
    fn info(&self) -> block::Info {
        block::Info {
            sectors: 80_000,
            readonly: false,
            flush: true,
        }
    }
    fn read_blocks(&mut self, lba: u64, b: &mut [u8]) -> Result<(), block::Error> {
        block::validate(self.info(), lba, b.len(), false)?;
        for (i, c) in b.chunks_mut(512).enumerate() {
            c.copy_from_slice(self.sectors.get(&(lba + i as u64)).unwrap_or(&[0; 512]));
        }
        Ok(())
    }
    fn write_blocks(&mut self, lba: u64, b: &[u8]) -> Result<(), block::Error> {
        block::validate(self.info(), lba, b.len(), true)?;
        for (i, c) in b.chunks(512).enumerate() {
            assert!((100..79_000).contains(&(lba + i as u64)));
            self.sectors.insert(lba + i as u64, c.try_into().unwrap());
            self.writes += 1;
        }
        Ok(())
    }
    fn flush(&mut self) -> Result<(), block::Error> {
        self.flushes += 1;
        Ok(())
    }
}
fn formatted() -> (Disk, gpt::Layout) {
    let guid = Guid::parse_or_panic("19e3b3b5-20ad-4d64-8ec4-e97b578589a0");
    let layout = gpt::Layout {
        data_start: 100,
        data_sectors: 78_900,
        esp_start: 40,
        esp_sectors: 60,
        data_readonly: false,
        primary_issue: None,
        backup_issue: None,
        disk_guid: guid,
        data_guid: guid,
    };
    let mut d = Disk {
        sectors: BTreeMap::new(),
        writes: 0,
        flushes: 0,
    };
    let state = State::default();
    let mut stream = Stream::new(block::Partition::open(&mut d, &layout).unwrap(), &state);
    fatfs::format_volume(
        &mut &mut stream,
        fatfs::FormatVolumeOptions::new()
            .fat_type(fatfs::FatType::Fat32)
            .bytes_per_cluster(512),
    )
    .unwrap();
    (d, layout)
}
#[test]
fn real_fat_backend_persists_reopens_and_rejects_alias_without_mutation() {
    let (mut d, layout) = formatted();
    let root = Path::root();
    {
        let mut s =
            FileService::new(fat::FatBackend::new(&mut d, layout), BootId([8; 16])).unwrap();
        let output = s
            .execute(&root, create("ReadMe.txt", "萝卜 test\n"))
            .unwrap()
            .result
            .unwrap();
        assert!(matches!(output, Output::Created(_)));
        let before = s.backend.device.writes;
        assert_eq!(
            s.execute(&root, create("README.TXT", "overwrite"))
                .unwrap()
                .result
                .unwrap_err()
                .code,
            Code::AlreadyExists
        );
        assert_eq!(s.backend.device.writes, before);
        assert_eq!(s.generation(), 1);
        assert_eq!(s.changes.iter().count(), 1);
        assert_eq!(
            s.execute(
                &root,
                Request::Read {
                    path: "missing",
                    offset: 0,
                    limit: 3,
                    reference: None
                }
            )
            .unwrap()
            .result
            .unwrap_err()
            .code,
            Code::NotFound
        );
        let output = s
            .execute(
                &root,
                Request::Read {
                    path: "readme.txt",
                    offset: 0,
                    limit: 64,
                    reference: None,
                },
            )
            .unwrap()
            .result
            .unwrap();
        assert_eq!(
            output,
            Output::Read {
                bytes: "萝卜 test\n".as_bytes().to_vec(),
                eof: true
            }
        );
    }
    let before = d.writes;
    let snapshot = d.sectors.clone();
    let mut s = FileService::new(fat::FatBackend::new(&mut d, layout), BootId([9; 16])).unwrap();
    assert_eq!(
        s.execute(
            &root,
            Request::Read {
                path: "ReadMe.txt",
                offset: 0,
                limit: 3,
                reference: None
            }
        )
        .unwrap()
        .result
        .unwrap(),
        Output::Read {
            bytes: "萝".as_bytes().to_vec(),
            eof: false
        }
    );
    drop(s);
    assert_eq!(d.writes, before);
    assert_eq!(d.sectors, snapshot);
}
#[test]
fn real_fat_sync_failures_remain_failed_with_changes_and_offline_state() {
    for stage in [Stage::FileSync, Stage::VolumeSync, Stage::DeviceFlush] {
        let (mut d, layout) = formatted();
        let mut backend = fat::FatBackend::new(&mut d, layout);
        backend.inject(stage);
        let mut s = FileService::new(backend, BootId([1; 16])).unwrap();
        let e = s
            .execute(&Path::root(), create("failed.txt", "payload"))
            .unwrap()
            .result
            .unwrap_err();
        assert_eq!(e.stage, stage);
        assert_eq!(e.partial_effect.written_bytes, 7);
        assert!(!e.partial_effect.persistence_confirmed);
        assert!(e.partial_effect.may_have_changed);
        assert_eq!(s.changes.iter().count(), 1);
        assert_eq!(s.volume().mount_state, MountState::Offline);
    }
}

#[test]
fn real_directory_pages_preserve_entries_and_read_limits_are_explicit() {
    let (mut disk, layout) = formatted();
    let root = Path::root();
    let mut s = FileService::new(fat::FatBackend::new(&mut disk, layout), BootId([1; 16])).unwrap();
    for i in 0..35 {
        assert!(
            s.execute(&root, create(&alloc::format!("entry-{i:02}.txt"), "x"))
                .unwrap()
                .result
                .is_ok()
        );
    }
    let Output::List(first) = s
        .execute(
            &root,
            Request::List {
                path: ".",
                cursor: None,
            },
        )
        .unwrap()
        .result
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(first.entries.len(), 32);
    let cursor = first.next.as_ref().unwrap();
    let Output::List(last) = s
        .execute(
            &root,
            Request::List {
                path: ".",
                cursor: Some(cursor),
            },
        )
        .unwrap()
        .result
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(last.entries.len(), 3);
    assert!(last.next.is_none());
    for (i, e) in first.entries.iter().chain(last.entries.iter()).enumerate() {
        assert_eq!(e.name, alloc::format!("entry-{i:02}.txt"));
        assert_eq!(e.size_bytes, Some(1));
    }
    assert_eq!(
        s.execute(
            &root,
            Request::Read {
                path: "entry-00.txt",
                offset: 2,
                limit: 1,
                reference: None
            }
        )
        .unwrap()
        .result
        .unwrap_err()
        .code,
        Code::InvalidArgument
    );
    assert_eq!(
        s.execute(
            &root,
            Request::Read {
                path: "entry-00.txt",
                offset: 1,
                limit: 1,
                reference: None
            }
        )
        .unwrap()
        .result
        .unwrap(),
        Output::Read {
            bytes: Vec::new(),
            eof: true
        }
    );
    let too_big = "x".repeat(MAX_BYTES + 1);
    let before = s.backend.device.writes;
    assert_eq!(
        s.execute(&root, create("large.txt", &too_big))
            .unwrap()
            .result
            .unwrap_err()
            .code,
        Code::ResourceLimit
    );
    assert_eq!(s.backend.device.writes, before);
}
#[test]
fn full_volume_scan_is_bounded_without_inventing_a_no_space_cause() {
    let (mut disk, layout) = formatted();
    // 独立修改格式化 fixture：依据 BPB 取得 FAT 边界，把全部可分配项标为已占用。
    let boot = disk.sectors.get(&100).unwrap();
    let reserved = u16::from_le_bytes(boot[14..16].try_into().unwrap()) as u64;
    let copies = boot[16] as u64;
    let sectors = u32::from_le_bytes(boot[36..40].try_into().unwrap()) as u64;
    for lba in 100 + reserved..100 + reserved + copies * sectors {
        let sector = disk.sectors.entry(lba).or_insert([0; 512]);
        for entry in sector.chunks_exact_mut(4) {
            if entry == [0; 4] {
                entry.copy_from_slice(&0x0fff_ffffu32.to_le_bytes());
            }
        }
    }
    let mut s = FileService::new(fat::FatBackend::new(&mut disk, layout), BootId([1; 16])).unwrap();
    let e = s
        .execute(&Path::root(), create("no-space.txt", "x"))
        .unwrap()
        .result
        .unwrap_err();
    assert_eq!(e.code, Code::ResourceLimit);
    assert!(matches!(
        e.cause,
        Cause::Stream(crate::storage::stream::Error {
            cause: crate::storage::stream::Cause::Budget,
            ..
        })
    ));
    assert_eq!(e.stage, Stage::Write);
    assert!(e.partial_effect.created);
    assert_eq!(e.partial_effect.written_bytes, 0);
    assert!(!e.partial_effect.persistence_confirmed);
    assert_eq!(s.changes.iter().count(), 1);
    assert_eq!(s.volume().mount_state, MountState::Offline);
}
