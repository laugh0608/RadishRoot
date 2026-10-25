#![no_std]
#![no_main]

use core::panic::PanicInfo;

// SAFETY: 本程序只导出一个 _start。T01 仅检查静态产物；调用方必须在
// T02 建立有效的 x86_64 执行环境、映射与栈，当前产物不能直接启动。
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
