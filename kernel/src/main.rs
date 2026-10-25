#![no_std]
#![no_main]
#![feature(abi_x86_interrupt, alloc_error_handler)]
extern crate alloc;

mod block_driver;
mod boot;
mod clock;
mod exclusive;
mod heap;
mod interrupts;
mod pci;
mod platform_memory;
mod runtime;
mod serial;
mod t03;
mod t04;
mod t05;
use core::{fmt::Write, panic::PanicInfo};

// SAFETY: 唯一导出入口，Limine 按 x86_64 System V ABI 提供有效栈及高半区映射。
// 只运行 BSP，异常与时钟准备好才启用定时 IRQ，不释放引导器内存，不返回引导器。
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // SAFETY: CPL0 下关闭中断；尚无 IDT / 中断处理，避免运行未建立的处理程序。
    unsafe {
        core::arch::asm!("cli", options(nomem, nostack));
    }
    interrupts::early();
    let Ok(mut serial) = serial::Serial::new() else {
        halt();
    };
    match boot::run(&mut serial) {
        Ok(facts) => match t03::run(&mut serial, &facts) {
            Ok(device) => {
                if let Err(code) = runtime::initialize(&mut serial, &facts) {
                    interrupts::fatal(format_args!("RR:T06:FAIL code={code} stage=initialize\n"));
                }
                if facts.mode.starts_with("t06-") {
                    runtime::test(&mut serial, device, facts.mode);
                }
                if facts.mode.starts_with("t05-") {
                    if let Err(error) = t05::run(&mut serial, device, facts.mode) {
                        let _ = writeln!(serial, "RR:T05:DETAIL error={error:?}");
                        let _ = writeln!(serial, "RR:T05:FAIL code={} stage=fat", error.code());
                    }
                } else if let Err(error) = t04::run(&mut serial, device, facts.mode) {
                    let _ = writeln!(serial, "RR:T04:DETAIL error={error:?}");
                    let _ = writeln!(serial, "RR:T04:FAIL code={} stage=storage", error.code());
                } else if facts.mode.is_empty() {
                    runtime::run(&mut serial, "", None);
                }
            }
            Err(code) => {
                let _ = writeln!(serial, "RR:T03:FAIL code={code} stage=platform");
            }
        },
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
    interrupts::fatal(format_args!(
        "\nRR:T02:FAIL code=BOOT_PANIC stage=panic detail={info}\n"
    ))
}
#[alloc_error_handler]
fn allocation_failure(layout: core::alloc::Layout) -> ! {
    interrupts::fatal(format_args!(
        "\nRR:T06:ALLOCATION size={} align={} last_heap_used={} snapshot=last-committed\nRR:T06:FAIL code=ALLOC_FAILURE stage=allocation\n",
        layout.size(),
        layout.align(),
        heap::last_used()
    ))
}
