//! T08 真实服务验收；只在 boot-test 模式接受专用 DATA 副本。
use crate::{block_driver::Driver, pci, platform_identity, serial::Serial};
use core::fmt::Write;
use radishroot_kernel::services::{fat::FatBackend, path::Path};
use radishroot_kernel::{services::*, storage::gpt};
const NAME: &str = "萝卜 服务.txt";
fn check(ok: bool, code: &'static str) -> Result<(), &'static str> {
    if ok { Ok(()) } else { Err(code) }
}
fn execute<B: Backend>(s: &mut FileService<B>, r: Request<'_>) -> Result<Reply, &'static str> {
    s.execute(&Path::root(), r).map_err(|_| "IDENTITY")
}
fn read<B: Backend>(
    s: &mut FileService<B>,
    path: &str,
    expected: &[u8],
) -> Result<(), &'static str> {
    let r = execute(
        s,
        Request::Read {
            path,
            offset: 0,
            limit: MAX_BYTES,
            reference: None,
        },
    )?;
    check(
        matches!(r.result,Ok(Output::Read{ref bytes,eof:true}) if bytes==expected),
        "READ_CONTENT",
    )
}
fn exercise(
    serial: &mut Serial,
    disk: &mut Driver,
    mode: &str,
    id: BootId,
) -> Result<(), &'static str> {
    let layout = gpt::inspect(disk).map_err(|_| "GPT")?;
    let mut backend = FatBackend::new(disk, layout);
    let injected = match mode {
        "t08-file-sync" => Some(Stage::FileSync),
        "t08-volume-sync" => Some(Stage::VolumeSync),
        "t08-device-flush" => Some(Stage::DeviceFlush),
        _ => None,
    };
    if let Some(stage) = injected {
        backend.inject(stage);
        writeln!(
            serial,
            "RR:T08:INJECTION stage={stage:?} mechanism=adapter-once"
        )
        .map_err(|_| "SERIAL")?;
    }
    let mut service = FileService::new(backend, id).map_err(|_| "SERVICE_ID")?;
    let payload = "RadishRoot T08 shared service\n".repeat(57);
    let failure = match mode {
        "t08-readonly" => Some((Code::ReadOnly, Stage::Validate)),
        "t08-bad-fat" => Some((Code::Corrupt, Stage::Mount)),
        _ => injected.map(|s| (Code::Io, s)),
    };
    if let Some((code, stage)) = failure {
        let r = execute(
            &mut service,
            if mode == "t08-bad-fat" {
                Request::List {
                    path: ".",
                    cursor: None,
                }
            } else {
                Request::CreateText {
                    path: "failed.txt",
                    text: &payload,
                }
            },
        )?;
        let e = r.result.map_or_else(Ok, |_| Err("UNEXPECTED_SUCCESS"))?;
        check(e.code == code && e.stage == stage, "WRONG_FAILURE")?;
        check(!e.partial_effect.persistence_confirmed, "FALSE_PERSISTENCE")?;
        let partial = injected.is_some();
        check(
            e.partial_effect.created == partial && e.partial_effect.may_have_changed == partial,
            "PARTIAL_STATE",
        )?;
        check(
            e.partial_effect.written_bytes == if partial { payload.len() } else { 0 },
            "WRITTEN",
        )?;
        check(
            service.generation() == u64::from(partial)
                && service.changes.iter().count() == usize::from(partial),
            "CHANGE_COUNT",
        )?;
        if mode != "t08-readonly" {
            let follow = execute(
                &mut service,
                Request::List {
                    path: ".",
                    cursor: None,
                },
            )?;
            check(
                follow.result.is_err_and(|e| e.code == Code::Offline),
                "OFFLINE",
            )?;
        } else {
            read(&mut service, "seed.txt", b"RadishRoot T08 seed\n")?;
        }
        writeln!(serial,"RR:T08:ERROR operation={} code={:?} stage={:?} created={} written={} changed={} persistence=0 cause={:?}",r.operation_id.sequence,e.code,e.stage,u8::from(e.partial_effect.created),e.partial_effect.written_bytes,u8::from(e.partial_effect.may_have_changed),e.cause).map_err(|_|"SERIAL")?;
    } else {
        let initial = execute(
            &mut service,
            Request::List {
                path: "/data",
                cursor: None,
            },
        )?
        .result
        .map_err(|_| "LIST")?;
        let Output::List(page) = initial else {
            return Err("LIST_TYPE");
        };
        check(
            page.next.is_none() && page.entries.len() == if mode == "t08-write" { 1 } else { 2 },
            "LIST_COUNT",
        )?;
        read(&mut service, "seed.txt", b"RadishRoot T08 seed\n")?;
        if mode == "t08-write" {
            for path in ["../escape", "/datax/escape", "bad:/../ok"] {
                check(
                    execute(&mut service, Request::CreateText { path, text: "bad" })?
                        .result
                        .is_err_and(|e| e.code == Code::InvalidPath),
                    "PATH_REJECTION",
                )?;
            }
            let r = execute(
                &mut service,
                Request::CreateText {
                    path: NAME,
                    text: &payload,
                },
            )?;
            check(matches!(r.result, Ok(Output::Created(_))), "CREATE")?;
            let record = service.operations.iter().last().unwrap();
            check(
                record.partial_effect.persistence_confirmed
                    && record.partial_effect.written_bytes == payload.len(),
                "PERSISTENCE",
            )?;
            let stages: alloc::vec::Vec<_> = service
                .events
                .iter()
                .filter(|e| e.operation_id == r.operation_id && e.status == Status::Flushing)
                .map(|e| e.stage)
                .collect();
            check(
                stages
                    == [
                        Stage::FileSync,
                        Stage::VolumeSync,
                        Stage::Unmount,
                        Stage::DeviceFlush,
                    ],
                "FLUSH_ORDER",
            )?;
            check(
                execute(
                    &mut service,
                    Request::CreateText {
                        path: "萝卜 服务.TXT",
                        text: "overwrite",
                    },
                )?
                .result
                .is_err_and(|e| e.code == Code::AlreadyExists),
                "COLLISION",
            )?;
            let reference = &page.entries[0].reference;
            check(
                execute(
                    &mut service,
                    Request::Read {
                        path: reference.path.as_str(),
                        offset: 0,
                        limit: 64,
                        reference: Some(reference),
                    },
                )?
                .result
                .is_err_and(|e| e.code == Code::StaleReference),
                "STALE",
            )?;
            check(
                service.generation() == 1 && service.changes.iter().count() == 1,
                "GENERATION",
            )?;
            writeln!(serial,"RR:T08:CREATE operation={} written={} flush_order=FileSync,VolumeSync,Unmount,DeviceFlush persistence=1 conflicts=1 stale=1 invalid=3",r.operation_id.sequence,payload.len()).map_err(|_|"SERIAL")?;
        }
        read(&mut service, NAME, payload.as_bytes())?;
        check(
            execute(
                &mut service,
                Request::Read {
                    path: "missing.txt",
                    offset: 0,
                    limit: 64,
                    reference: None,
                },
            )?
            .result
            .is_err_and(|e| e.code == Code::NotFound),
            "NOT_FOUND",
        )?;
        read(&mut service, "seed.txt", b"RadishRoot T08 seed\n")?;
        writeln!(
            serial,
            "RR:T08:CONTENT files=2 bytes={} missing_recovered=1",
            payload.len()
        )
        .map_err(|_| "SERIAL")?;
    }
    let events: alloc::vec::Vec<_> = service.events.iter().map(|e| e.seq).collect();
    check(events.windows(2).all(|p| p[0] + 1 == p[1]), "EVENT_ORDER")?;
    writeln!(
        serial,
        "RR:T08:RESULT generation={} changes={} records={} state={:?} expected_failure={}",
        service.generation(),
        service.changes.iter().count(),
        service.operations.iter().count(),
        service.volume().mount_state,
        u8::from(failure.is_some())
    )
    .map_err(|_| "SERIAL")?;
    // 明确序列化结构化记录，不把调试描述作为未来 shell 数据协议。
    for r in service.operations.iter() {
        writeln!(
            serial,
            "RR:T08:OP id={} kind={:?} stage={:?} status={:?} last={:?}",
            r.id.sequence, r.request_kind, r.stage, r.status, r.last_completed
        )
        .map_err(|_| "SERIAL")?;
    }
    Ok(())
}
pub fn run(serial: &mut Serial, device: pci::Device, mode: &str) {
    let result = (|| {
        let id = platform_identity::read()?;
        writeln!(
            serial,
            "RR:T08:ENTER mode={mode} boot_id={:032x}",
            u128::from_be_bytes(id.0)
        )
        .map_err(|_| "SERIAL")?;
        let mut disk = Driver::new(device, false).map_err(|_| "DEVICE")?;
        let result = exercise(serial, &mut disk, mode, id);
        disk.stop().map_err(|_| "DEVICE_STOP")?;
        writeln!(serial,"RR:T08:STOP reset=confirmed bus_master=0 allocations=0 shared=0 requests={} flushes={}",disk.requests,disk.flushes).map_err(|_|"SERIAL")?;
        result?;
        writeln!(serial, "RR:T08:READY mode={mode}").map_err(|_| "SERIAL")
    })();
    if let Err(code) = result {
        let _ = writeln!(serial, "RR:T08:FAIL code={code} stage=acceptance");
    }
}
