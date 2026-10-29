//! 专用页面所有者与唯一 framebuffer 写入口，复用引导映射，不另造设备缓存别名。
use super::{MEMORY, pointer};
use crate::boot::Facts;
use core::ptr::NonNull;
use radishroot_kernel::{
    memory::frames::{Block, LIMIT, PAGE, Purpose},
    surface::{Format, SENTINEL, SENTINEL_BYTES, Surface},
};
use x86_64::{
    VirtAddr,
    registers::control::Cr3,
    structures::paging::{PageTable, PageTableFlags},
};

pub struct Display {
    block: Block,
    ram: NonNull<u8>,
    framebuffer: NonNull<u8>,
    pub format: Format,
    pub presents: u64,
}
impl Display {
    pub fn new(
        facts: &Facts,
        fail_allocation: bool,
        bad_description: bool,
    ) -> Result<Self, &'static str> {
        let mut description = facts.framebuffer;
        if bad_description {
            description.pitch = 0;
        }
        let format = Format::new(description).map_err(|e| e.code())?;
        if facts.framebuffer_address
            != facts
                .hhdm
                .checked_add(description.physical)
                .ok_or("SURFACE_MAPPING")?
        {
            return Err("SURFACE_MAPPING");
        }
        let framebuffer =
            NonNull::new(facts.framebuffer_address as *mut u8).ok_or("SURFACE_MAPPING")?;
        let (block, ram) = {
            let mut memory = MEMORY.lock();
            if !memory.tables_checked {
                return Err("SURFACE_TABLES");
            }
            validate_mapping(
                facts.framebuffer_address,
                description.physical,
                format.length,
                memory.hhdm,
            )?;
            let block = memory
                .frames
                .allocate_below(
                    format.geometry.allocation_bytes() / 4096,
                    1,
                    Purpose::Surface,
                    if fail_allocation { 1024 * 1024 } else { LIMIT },
                )
                .map_err(|_| "SURFACE_ALLOCATION")?;
            let ram = match pointer(memory.hhdm, block.base, block.pages * 4096) {
                Ok(p) => p,
                Err(e) => {
                    memory
                        .frames
                        .release(block)
                        .map_err(|_| "SURFACE_RELEASE")?;
                    return Err(e);
                }
            };
            (block, ram)
        };
        // SAFETY: 新分配的连续 usable RAM 独占且无 DMA、无 Rust 引用；HHDM 映射终生保持。
        // 锁已释放，清零允许 timer IRQ；实际像素前后各保留完整一页哨兵，页尾余量也保护。
        unsafe {
            ram.as_ptr().write_bytes(SENTINEL, block.pages * 4096);
            ram.as_ptr()
                .add(SENTINEL_BYTES)
                .write_bytes(0, format.geometry.bytes());
        }
        Ok(Self {
            block,
            ram,
            framebuffer,
            format,
            presents: 0,
        })
    }
    pub fn surface(&mut self) -> Surface<'_> {
        // SAFETY: RAM 对齐 PAGE；偏移一页保持 u32 对齐；独占借用 self 排除 present/释放/别名。
        // slice 只包含真实像素，生命周期不超过本所有者借用；设备映射从不构造成 slice。
        let pixels = unsafe {
            core::slice::from_raw_parts_mut(
                self.ram.as_ptr().add(SENTINEL_BYTES).cast::<u32>(),
                self.format.geometry.pixels(),
            )
        };
        Surface::new(self.format.geometry, pixels).expect("validated surface capacity")
    }
    pub fn check(&self) -> Result<(), &'static str> {
        let pixel_end = SENTINEL_BYTES + self.format.geometry.bytes();
        // SAFETY: 只读所有者分配内的哨兵和页尾，前一绘制借用已结束，无设备访问 RAM。
        unsafe {
            for i in (0..SENTINEL_BYTES).chain(pixel_end..self.block.pages * PAGE as usize) {
                if self.ram.as_ptr().add(i).read() != SENTINEL {
                    return Err("SURFACE_SENTINEL");
                }
            }
        }
        Ok(())
    }
    pub fn present(&mut self) -> Result<(), &'static str> {
        self.check()?;
        let next = self.presents.checked_add(1).ok_or("SURFACE_GENERATION")?;
        let width = self.format.geometry.width() as usize;
        // SAFETY: 所有权与映射已经验证，主循环唯一写设备方；无设备内存 Rust 引用。
        // 源为独占 RAM 像素；每行只写 width*4 字节，不接触 pitch padding；逐字节 volatile
        // 不依赖设备地址或 pitch 的 u32 对齐；IRQ 不访问这两份缓冲区。
        unsafe {
            let source = self.ram.as_ptr().add(SENTINEL_BYTES).cast::<u32>();
            for y in 0..self.format.geometry.height() as usize {
                for x in 0..width {
                    let bytes = self.format.pack(source.add(y * width + x).read());
                    let address = self.framebuffer.as_ptr().add(y * self.format.pitch + x * 4);
                    for (i, byte) in bytes.into_iter().enumerate() {
                        address.add(i).write_volatile(byte);
                    }
                }
            }
        }
        self.presents = next;
        Ok(())
    }
    pub fn page_bytes(&self) -> usize {
        self.block.pages * 4096
    }
}
impl Drop for Display {
    fn drop(&mut self) {
        // &mut self 排除所有 surface 借用；后台没有提交 DMA，不归还 framebuffer 映射。
        MEMORY
            .lock()
            .frames
            .release(self.block)
            .expect("surface ownership ledger");
    }
}
fn validate_mapping(
    virtual_start: u64,
    physical_start: u64,
    length: usize,
    hhdm: u64,
) -> Result<(), &'static str> {
    let end = virtual_start
        .checked_add(length as u64)
        .ok_or("SURFACE_MAPPING")?;
    VirtAddr::try_new(virtual_start).map_err(|_| "SURFACE_MAPPING")?;
    VirtAddr::try_new(end - 1).map_err(|_| "SURFACE_MAPPING")?;
    let first = virtual_start & !4095;
    let physical_first = physical_start
        .checked_sub(virtual_start - first)
        .ok_or("SURFACE_MAPPING")?;
    for address in (first..end).step_by(4096) {
        let mut table = Cr3::read().0.start_address().as_u64();
        let mut resolved = None;
        for shift in [39, 30, 21, 12] {
            let p = pointer(hhdm, table, 4096)?;
            // SAFETY: 既有页表在平台初始化中已验证；短临界区 IF=0 单 BSP；只读每层条目，
            // 不持有跨层可变引用，不创建或改变 framebuffer 映射及缓存属性。
            let entries = unsafe { &*p.as_ptr().cast::<PageTable>() };
            let entry = &entries[((address >> shift) & 511) as usize];
            let flags = entry.flags();
            if !flags.contains(PageTableFlags::PRESENT | PageTableFlags::WRITABLE) {
                return Err("SURFACE_MAPPING");
            }
            if shift == 12 || flags.contains(PageTableFlags::HUGE_PAGE) {
                if shift == 39 {
                    return Err("SURFACE_MAPPING");
                }
                let mask = (1u64 << shift) - 1;
                resolved = Some((entry.addr().as_u64() & !mask) + (address & mask));
                break;
            }
            table = entry.addr().as_u64();
        }
        if resolved != Some(physical_first + address - first) {
            return Err("SURFACE_MAPPING");
        }
    }
    Ok(())
}
