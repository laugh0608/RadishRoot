//! 默认路径使用真实 DATA 服务；不创建演示文件，不引入宿主替身。
use crate::{block_driver::Driver, boot::Facts, platform_identity, serial::Serial};
use core::fmt::Write;
use radishroot_kernel::{
    services::{FileService, Output, Request, fat::FatBackend, path::Path},
    storage::gpt,
};
pub type Service<'a> = FileService<FatBackend<'a, Driver>>;
pub fn run(serial: &mut Serial, mut disk: Driver, facts: &Facts) -> ! {
    let id = platform_identity::read().unwrap_or_else(|code| {
        crate::interrupts::fatal(format_args!("RR:T08:FAIL code={code} stage=identity\n"))
    });
    let layout = gpt::inspect(&mut disk).unwrap_or_else(|e| {
        crate::interrupts::fatal(format_args!("RR:T08:FAIL code={} stage=gpt\n", e.code()))
    });
    let backend = FatBackend::new(&mut disk, layout);
    #[cfg(feature = "boot-test")]
    let backend = {
        let mut backend = backend;
        if facts.mode == "t10-file-sync" {
            backend.inject(radishroot_kernel::services::Stage::FileSync);
            let _ = writeln!(
                serial,
                "RR:T10:INJECTION stage=FileSync mechanism=adapter-once"
            );
        }
        backend
    };
    let mut files = FileService::new(backend, id).expect("verified nonzero identities");
    let reply = files
        .execute(
            &Path::root(),
            Request::List {
                path: "/data",
                cursor: None,
            },
        )
        .expect("initial operation identity");
    let page = match reply.result {
        Ok(Output::List(page)) => Some(page),
        Err(e) => {
            let _ = writeln!(
                serial,
                "RR:T08:DEFAULT_ERROR code={:?} stage={:?} cause={:?}",
                e.code, e.stage, e.cause
            );
            None
        }
        _ => unreachable!(),
    };
    let _ = writeln!(
        serial,
        "RR:T08:DEFAULT entries={} more={} readonly={} generation={} operation={} boot_id={:032x}",
        page.as_ref().map_or(0, |p| p.entries.len()),
        u8::from(page.as_ref().is_some_and(|p| p.next.is_some())),
        u8::from(files.volume().read_only),
        files.generation(),
        reply.operation_id.sequence,
        u128::from_be_bytes(id.0)
    );
    crate::runtime::run(serial, "", None, Some(facts), Some(&mut files));
}
