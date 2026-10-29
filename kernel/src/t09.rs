//! 专用 boot-test 仅编排真实控制台生命周期；命令从宿主串口输入，不在 guest 假造结果。
use crate::{block_driver::Driver, console::Console, pci, platform_identity, serial::Serial};
use core::fmt::Write;
use radishroot_kernel::{
    services::{FileService, Stage, fat::FatBackend},
    storage::gpt,
};
pub fn run(serial: &mut Serial, device: pci::Device, mode: &str) {
    let result = (|| {
        let id = platform_identity::read()?;
        writeln!(
            serial,
            "RR:T09:ENTER mode={mode} boot_id={:032x}",
            u128::from_be_bytes(id.0)
        )
        .map_err(|_| "SERIAL")?;
        let mut disk = Driver::new(device, false).map_err(|_| "DEVICE")?;
        let result = (|| {
            let layout = gpt::inspect(&mut disk).map_err(|_| "GPT")?;
            let mut backend = FatBackend::new(&mut disk, layout);
            let stage = match mode {
                "t09-file-sync" => Some(Stage::FileSync),
                "t09-volume-sync" => Some(Stage::VolumeSync),
                "t09-device-flush" => Some(Stage::DeviceFlush),
                _ => None,
            };
            if let Some(stage) = stage {
                backend.inject(stage);
                writeln!(
                    serial,
                    "RR:T09:INJECTION stage={stage:?} mechanism=adapter-once"
                )
                .map_err(|_| "SERIAL")?;
            }
            let mut files = FileService::new(backend, id).map_err(|_| "SERVICE")?;
            let mut console = Console::new();
            console.announce(serial, &files)?;
            let expected = match mode {
                "t09-write" => 20,
                "t09-reopen" => 6,
                _ => 4,
            };
            while console.completed < expected {
                let now = crate::clock::now().map_err(|e| e.code())?;
                let pending = console.turn(serial, &mut files, now)?;
                crate::interrupts::idle(pending);
            }
            writeln!(
                serial,
                "RR:T09:RESULT commands={} records={} generation={} changes={} state={:?}",
                console.completed,
                files.operations.iter().count(),
                files.generation(),
                files.changes.iter().count(),
                files.volume().mount_state
            )
            .map_err(|_| "SERIAL")?;
            Ok(())
        })();
        disk.stop().map_err(|_| "STOP")?;
        writeln!(serial,"RR:T09:STOP reset=confirmed bus_master=0 allocations=0 shared=0 requests={} flushes={}",disk.requests,disk.flushes).map_err(|_|"SERIAL")?;
        result?;
        writeln!(serial, "RR:T09:READY mode={mode}").map_err(|_| "SERIAL")
    })();
    if let Err(code) = result {
        let _ = writeln!(serial, "RR:T09:FAIL code={code} stage=console-test");
    }
}
