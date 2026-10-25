#![no_std]
#![no_main]

mod boot;
mod exclusive;
mod heap;
mod pci;
mod platform_memory;
mod serial;
mod t03;
use core::{
    fmt::Write,
    panic::PanicInfo,
    sync::atomic::{AtomicBool, Ordering},
};

// SAFETY: 唯一导出入口，Limine 按 x86_64 System V ABI 提供有效栈及高半区映射。
// 本阶段只运行 BSP，不启用中断，不释放引导器提供的内存，不返回引导器。
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // SAFETY: CPL0 下关闭中断；尚无 IDT / 中断处理，避免运行未建立的处理程序。
    unsafe {
        core::arch::asm!("cli", options(nomem, nostack));
    }
    let Ok(mut serial) = serial::Serial::new() else {
        halt();
    };
    match boot::run(&mut serial) {
        Ok(facts) => {
            if let Err(code) = t03::run(&mut serial, &facts) {
                let _ = writeln!(serial, "RR:T03:FAIL code={code} stage=platform");
            }
        }
        Err(error) => {
            let _ = writeln!(
                serial,
                "RR:T02:FAIL code={} stage=boot-validation",
                error.code()
            );
        }
    }
    halt()
}
pub fn halt() -> ! {
    loop {
        // SAFETY: CPL0、唯一 BSP；IF=0 后停止 CPU，不释放仍有效的资源，不伪报关机。
        unsafe {
            core::arch::asm!("cli", "hlt", options(nomem, nostack));
        }
    }
}
#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    static PANICKING: AtomicBool = AtomicBool::new(false);
    if !PANICKING.swap(true, Ordering::Relaxed)
        && let Ok(mut serial) = serial::Serial::new()
    {
        let _ = writeln!(
            serial,
            "RR:T02:FAIL code=BOOT_PANIC stage=panic detail={info}"
        );
    }
    halt()
}
