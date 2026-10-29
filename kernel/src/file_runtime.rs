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
    let mut files = FileService::new(FatBackend::new(&mut disk, layout), id)
        .expect("verified nonzero identities");
    let reply = files
        .execute(
            &Path::root(),
            Request::List {
                path: "/data",
                cursor: None,
            },
        )
        .expect("initial operation identity");
    let Output::List(page) = reply.result.unwrap_or_else(|e| {
        crate::interrupts::fatal(format_args!(
            "RR:T08:FAIL code={:?} stage={:?} cause={:?}\n",
            e.code, e.stage, e.cause
        ))
    }) else {
        unreachable!()
    };
    let _ = writeln!(
        serial,
        "RR:T08:DEFAULT entries={} more={} readonly={} generation={} operation={} boot_id={:032x}",
        page.entries.len(),
        u8::from(page.next.is_some()),
        u8::from(files.volume().read_only),
        files.generation(),
        reply.operation_id.sequence,
        u128::from_be_bytes(id.0)
    );
    crate::runtime::run(serial, "", None, Some(facts), Some(&mut files));
}
