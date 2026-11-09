//! 固定 q35 fw_cfg 只读启动身份；不使用 DMA，不把时钟值当成唯一身份。
use radishroot_kernel::services::BootId;
use x86_64::instructions::{interrupts, port::Port};
fn select(key: u16) {
    // SAFETY: 固定 QEMU x86 q35，CPL0，单 BSP / IF=0；选择只读通用项，无 DMA。
    unsafe { Port::<u16>::new(0x510).write(key) }
}
fn bytes<const N: usize>() -> [u8; N] {
    let mut output = [0; N];
    for byte in &mut output {
        // SAFETY: 同 select 的独占临界区，0x511 为 fw_cfg 数据端口；次数由 N 有界。
        *byte = unsafe { Port::<u8>::new(0x511).read() };
    }
    output
}
pub fn read() -> Result<BootId, &'static str> {
    interrupts::without_interrupts(|| {
        select(0);
        if bytes::<4>() != *b"QEMU" {
            return Err("BOOT_ID_DEVICE");
        }
        select(0x19);
        let count = u32::from_be_bytes(bytes());
        if count > 256 {
            return Err("BOOT_ID_DIRECTORY_LIMIT");
        }
        let mut found = None;
        for _ in 0..count {
            let entry = bytes::<64>();
            let name = b"opt/radishroot/boot-id\0";
            if entry[8..].starts_with(name) {
                let selector = u16::from_be_bytes([entry[4], entry[5]]);
                if found.is_some()
                    || u32::from_be_bytes(entry[..4].try_into().unwrap()) != 16
                    || !(0x20..0x4000).contains(&selector)
                {
                    return Err("BOOT_ID_ENTRY");
                }
                found = Some(selector);
            }
        }
        select(found.ok_or("BOOT_ID_MISSING")?);
        let id = bytes::<16>();
        if id == [0; 16] {
            return Err("BOOT_ID_ZERO");
        }
        Ok(BootId(id))
    })
}
