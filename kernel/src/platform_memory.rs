use crate::{exclusive::Exclusive, heap};
use core::ptr::NonNull;
use radishroot_kernel::{
    boot_contract::Region,
    memory::{
        dma::{Direction, Ledger, Token},
        frames::{Block, Frames, PAGE, Purpose},
    },
    pci_contract::Bar,
};
use virtio_drivers::{BufferDirection, Hal, PhysAddr};
use x86_64::{
    PhysAddr as CpuPhys, VirtAddr,
    registers::control::{Cr3, Cr4, Cr4Flags},
    registers::model_specific::{Efer, EferFlags, Msr},
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame,
        Size4KiB, Translate,
    },
};
const MMIO_BASE: u64 = 0xffffc00000000000;
#[derive(Clone, Copy)]
struct Shared {
    token: Token,
    block: Block,
    original: usize,
    length: usize,
    direction: Direction,
}
struct Memory {
    frames: Frames,
    hhdm: u64,
    mmio: [Bar; 6],
    claims: [Bar; 16],
    blocks: [Option<Block>; 64],
    shared: [Option<Shared>; 64],
    ledger: Ledger,
}
static MEMORY: Exclusive<Memory> = Exclusive::new(Memory {
    frames: Frames::new(),
    hhdm: 0,
    mmio: [Bar { base: 0, length: 0 }; 6],
    claims: [Bar { base: 0, length: 0 }; 16],
    blocks: [None; 64],
    shared: [None; 64],
    ledger: Ledger::new(),
});
fn pointer(offset: u64, base: u64, length: usize) -> Result<NonNull<u8>, &'static str> {
    if length == 0 {
        return Err("MEM_RANGE");
    }
    let address = offset.checked_add(base).ok_or("MEM_RANGE")?;
    let end = address.checked_add(length as u64 - 1).ok_or("MEM_RANGE")?;
    VirtAddr::try_new(address).map_err(|_| "MEM_MAPPING")?;
    VirtAddr::try_new(end).map_err(|_| "MEM_MAPPING")?;
    NonNull::new(address as *mut u8).ok_or("MEM_MAPPING")
}
pub fn initialize(regions: &[Region], hhdm: u64) -> Result<(usize, u64), &'static str> {
    if Cr4::read().contains(Cr4Flags::L5_PAGING) {
        return Err("MEM_PAGING_MODE");
    }
    let mut m = MEMORY.lock();
    m.frames.init(regions).map_err(|_| "MEM_MAP")?;
    m.hhdm = hhdm;
    let pages = m.frames.available();
    let b = m
        .frames
        .allocate(256, 1, Purpose::Heap)
        .map_err(|_| "MEM_HEAP")?;
    let p = pointer(hhdm, b.base, b.pages * PAGE as usize)?;
    // SAFETY: frames 独占分配的 1 MiB usable RAM，HHDM 由 Limine 保留，永不归还此堆。
    unsafe {
        core::ptr::write_bytes(p.as_ptr(), 0, b.pages * PAGE as usize);
        heap::init(p.as_ptr(), b.pages * PAGE as usize);
    }
    Ok((pages, b.base))
}
pub fn page_check() -> Result<u64, &'static str> {
    let mut m = MEMORY.lock();
    let before = m.frames.available();
    let b = m
        .frames
        .allocate(2, 4, Purpose::Test)
        .map_err(|_| "MEM_ALLOC")?;
    let p = pointer(m.hhdm, b.base, 8192)?;
    // SAFETY: 两页当前只属于本测试，HHDM 映射不变；无 DMA / 别名，释放前结束全部访问。
    unsafe {
        for i in 0..8192 {
            p.as_ptr().add(i).write_volatile((i as u8).wrapping_mul(17));
        }
        for i in 0..8192 {
            if p.as_ptr().add(i).read_volatile() != (i as u8).wrapping_mul(17) {
                return Err("MEM_READBACK");
            }
        }
    }
    m.frames.release(b).map_err(|_| "MEM_RELEASE")?;
    if m.frames.available() != before {
        return Err("MEM_ACCOUNTING");
    }
    Ok(b.base)
}
struct TableFrames<'a> {
    frames: &'a mut Frames,
    hhdm: u64,
}
// SAFETY: 每次返回不同的 usable 页，记录为 PageTable 且不释放；预先清零，无存活别名。
unsafe impl FrameAllocator<Size4KiB> for TableFrames<'_> {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let b = self.frames.allocate(1, 1, Purpose::PageTable).ok()?;
        let p = pointer(self.hhdm, b.base, 4096).ok()?;
        // SAFETY: 此页刚独占分配，未发布到当前页表，后续全部访问归 mapper 管理。
        unsafe {
            p.as_ptr().write_bytes(0, 4096);
        }
        PhysFrame::from_start_address(CpuPhys::new(b.base)).ok()
    }
}
fn check_tables(
    physical: u64,
    level: u8,
    hhdm: u64,
    regions: &[Region],
    budget: &mut usize,
) -> Result<(), &'static str> {
    if *budget == 0
        || !physical.is_multiple_of(4096)
        || !regions.iter().any(|r| {
            r.kind == 5
                && r.base <= physical
                && physical
                    .checked_add(4096)
                    .is_some_and(|e| e <= r.base + r.length)
        })
    {
        return Err("MEM_TABLE_RANGE");
    }
    *budget -= 1;
    let p = pointer(hhdm, physical, 4096)?;
    // SAFETY: 受信引导器给出的页表页在 bootloader-reclaimable 映射中；此时只读且 IF=0。
    let table = unsafe { &*p.as_ptr().cast::<PageTable>() };
    if level > 1 {
        for entry in table.iter().filter(|e| {
            e.flags().contains(PageTableFlags::PRESENT)
                && !e.flags().contains(PageTableFlags::HUGE_PAGE)
        }) {
            check_tables(entry.addr().as_u64(), level - 1, hhdm, regions, budget)?;
        }
    }
    Ok(())
}
pub fn map_bars(bars: [Bar; 6], regions: &[Region]) -> Result<(), &'static str> {
    radishroot_kernel::pci_contract::bars(&bars, regions).map_err(|_| "PCI_BAR_RANGE")?;
    let mut m = MEMORY.lock();
    if m.mmio.iter().any(|b| b.length != 0) {
        return Err("MMIO_REINITIALIZE");
    }
    let hhdm = m.hhdm;
    let physical = Cr3::read().0.start_address().as_u64();
    check_tables(physical, 4, hhdm, regions, &mut 4096)?;
    if !Efer::read().contains(EferFlags::NO_EXECUTE_ENABLE) {
        return Err("MMIO_NX_DISABLED");
    }
    // SAFETY: CPL0；IA32_PAT 标准寄存器可读，当前 x86_64 QEMU 支持 PAT。
    let pat = unsafe { Msr::new(0x277).read() };
    if (pat >> 24) & 255 != 0 {
        return Err("MMIO_CACHE_MODE");
    }
    let p = pointer(hhdm, physical, 4096)?;
    // SAFETY: 唯一 BSP / IF=0；所有已有页表已只读检查，不持有其引用。映射只改新 MMIO 窗口。
    // 新页表由 TableFrames 分配为永久保留 RAM；HHDM 覆盖全部已有与新增页表页。
    let mut mapper =
        unsafe { OffsetPageTable::new(&mut *p.as_ptr().cast::<PageTable>(), VirtAddr::new(hhdm)) };
    let mut allocator = TableFrames {
        frames: &mut m.frames,
        hhdm,
    };
    for (index, b) in bars.iter().enumerate().filter(|(_, b)| b.length != 0) {
        let virtual_base = MMIO_BASE + index as u64 * 16 * 1024 * 1024;
        for offset in (0..b.length).step_by(4096) {
            if let Some(alias) = hhdm
                .checked_add(b.base + offset)
                .and_then(|a| VirtAddr::try_new(a).ok())
                && mapper.translate_addr(alias).is_some()
            {
                return Err("MMIO_HHDM_ALIAS");
            }
            let addr = VirtAddr::new(virtual_base + offset);
            if mapper.translate_addr(addr).is_some() {
                return Err("MMIO_ALREADY_MAPPED");
            }
            let frame = PhysFrame::<Size4KiB>::from_start_address(CpuPhys::new(b.base + offset))
                .map_err(|_| "MMIO_ALIGN")?;
            let flags = PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::NO_CACHE
                | PageTableFlags::WRITE_THROUGH
                | PageTableFlags::NO_EXECUTE;
            // SAFETY: 固定 BAR 与 RAM / 其他区域不重叠，目的窗口未映射，仅由 HAL 发放唯一范围。
            unsafe {
                mapper
                    .map_to(Page::containing_address(addr), frame, flags, &mut allocator)
                    .map_err(|_| "MMIO_MAP")?
                    .flush();
            }
            if mapper.translate_addr(addr) != Some(CpuPhys::new(b.base + offset)) {
                return Err("MMIO_READBACK");
            }
        }
    }
    m.mmio = bars;
    Ok(())
}
fn direction(d: BufferDirection) -> Direction {
    match d {
        BufferDirection::DriverToDevice => Direction::ToDevice,
        BufferDirection::DeviceToDriver => Direction::FromDevice,
        BufferDirection::Both => Direction::Both,
    }
}
pub struct VirtioHal;
// SAFETY: RAM 分配唯一记账 / 清零；MMIO 只发放已映射且互不重叠的 BAR 范围；share 采用连续
// bounce RAM，不依赖原缓冲区连续性。T03 设备 bus master 关闭且无队列；所有失败立即停止。
// 后续提交必须通过下述 ledger 转换，完成 / 复位确认以前禁止 unshare 或队列内存回收。
unsafe impl Hal for VirtioHal {
    fn dma_alloc(pages: usize, _: BufferDirection) -> (PhysAddr, NonNull<u8>) {
        let mut m = MEMORY.lock();
        let slot = m
            .blocks
            .iter()
            .position(Option::is_none)
            .expect("T03 DMA records exhausted");
        let b = m
            .frames
            .allocate(pages, 1, Purpose::Dma)
            .expect("T03 DMA allocation rejected");
        let p = pointer(m.hhdm, b.base, pages * 4096).expect("T03 DMA mapping");
        // SAFETY: 此处刚分配的连续 RAM 无存活引用，之后只由返回指针访问，直到精确释放。
        unsafe {
            p.as_ptr().write_bytes(0, pages * 4096);
        }
        m.blocks[slot] = Some(b);
        (b.base, p)
    }
    unsafe fn dma_dealloc(physical: PhysAddr, virtual_address: NonNull<u8>, pages: usize) -> i32 {
        let mut m = MEMORY.lock();
        let Some(slot) = m
            .blocks
            .iter()
            .position(|b| b.is_some_and(|b| b.base == physical && b.pages == pages))
        else {
            return -1;
        };
        let b = m.blocks[slot].unwrap();
        if pointer(m.hhdm, physical, pages * 4096).ok() != Some(virtual_address) {
            return -1;
        }
        // 原缓冲 / bounce 有共享记录时拒绝归还；包括 timeout 后的隔离记录。
        if m.shared.iter().flatten().any(|s| {
            s.block == b
                || (s.original < virtual_address.as_ptr() as usize + pages * 4096
                    && (virtual_address.as_ptr() as usize) < s.original + s.length)
        }) {
            return -2;
        }
        if m.frames.release(b).is_err() {
            return -1;
        }
        m.blocks[slot] = None;
        0
    }
    unsafe fn mmio_phys_to_virt(physical: PhysAddr, size: usize) -> NonNull<u8> {
        let mut m = MEMORY.lock();
        let index = m
            .mmio
            .iter()
            .position(|b| b.contains(physical, size))
            .expect("T03 MMIO outside verified BAR");
        assert!(
            !m.claims.iter().any(|b| b.length != 0
                && physical < b.base + b.length
                && b.base < physical + size as u64),
            "T03 MMIO alias"
        );
        let slot = m
            .claims
            .iter()
            .position(|b| b.length == 0)
            .expect("T03 MMIO claims full");
        m.claims[slot] = Bar {
            base: physical,
            length: size as u64,
        };
        NonNull::new(
            (MMIO_BASE + index as u64 * 16 * 1024 * 1024 + physical - m.mmio[index].base)
                as *mut u8,
        )
        .unwrap()
    }
    unsafe fn share(buffer: NonNull<[u8]>, d: BufferDirection) -> PhysAddr {
        let original = buffer.as_ptr() as *mut u8 as usize;
        let length = buffer.len();
        assert!(length > 0 && length <= 1024 * 1024, "T03 DMA share length");
        let mut m = MEMORY.lock();
        let slot = m
            .shared
            .iter()
            .position(Option::is_none)
            .expect("T03 DMA shares full");
        let block = m
            .frames
            .allocate(length.div_ceil(4096), 1, Purpose::Dma)
            .expect("T03 bounce allocation");
        let p = pointer(m.hhdm, block.base, block.pages * 4096).expect("T03 bounce mapping");
        let dir = direction(d);
        let token = m
            .ledger
            .share(block.base, original, length, dir)
            .expect("T03 overlapping share");
        // SAFETY: trait 调用者给出非空有效源；bounce 是新独占页，不与源重叠；本阶段无设备访问。
        unsafe {
            p.as_ptr().write_bytes(0, block.pages * 4096);
            if dir != Direction::FromDevice {
                core::ptr::copy_nonoverlapping(original as *const u8, p.as_ptr(), length);
            }
        }
        m.shared[slot] = Some(Shared {
            token,
            block,
            original,
            length,
            direction: dir,
        });
        block.base
    }
    unsafe fn unshare(physical: PhysAddr, buffer: NonNull<[u8]>, d: BufferDirection) {
        let mut m = MEMORY.lock();
        let original = buffer.as_ptr() as *mut u8 as usize;
        let length = buffer.len();
        let dir = direction(d);
        let token = m
            .ledger
            .identify(physical, original, length, dir)
            .expect("T03 DMA identity mismatch");
        // 状态拒绝必须发生在 copy-back / 释放以前；Quarantined 不能复用原缓冲区。
        m.ledger.release(token).expect("T03 DMA still device-owned");
        let slot = m
            .shared
            .iter()
            .position(|s| s.is_some_and(|s| s.token == token))
            .expect("T03 share record");
        let share = m.shared[slot].unwrap();
        assert_eq!(share.direction, dir);
        let p = pointer(m.hhdm, physical, length).expect("T03 bounce mapping");
        // SAFETY: 状态机已经确认未提交或完成；caller 保留原缓冲独占和生命周期，源为本记录 bounce。
        unsafe {
            if dir != Direction::ToDevice {
                core::ptr::copy_nonoverlapping(p.as_ptr(), original as *mut u8, length);
            }
        }
        m.frames.release(share.block).expect("T03 bounce release");
        m.shared[slot] = None;
    }
}
pub fn dma_transition(physical: u64, submit: bool) -> Result<(), &'static str> {
    let mut m = MEMORY.lock();
    let token = m
        .shared
        .iter()
        .flatten()
        .find(|s| s.block.base == physical)
        .ok_or("DMA_UNKNOWN")?
        .token;
    if submit {
        m.ledger.submit(token).map_err(|_| "DMA_STATE")
    } else {
        m.ledger.timeout(token).map_err(|_| "DMA_STATE")
    }
}
pub fn reject_busy_unshare(physical: u64) -> Result<(), &'static str> {
    let mut m = MEMORY.lock();
    let token = m
        .shared
        .iter()
        .flatten()
        .find(|s| s.block.base == physical)
        .ok_or("DMA_UNKNOWN")?
        .token;
    m.ledger.release(token).map_err(|_| "DMA_IN_FLIGHT")
}

pub fn reject_invalid_allocation() -> Result<(), &'static str> {
    let mut memory = MEMORY.lock();
    match memory.frames.allocate(0, 1, Purpose::Test) {
        Err(radishroot_kernel::memory::frames::Error::Range) => Err("MEM_RANGE"),
        _ => Err("TEST_UNEXPECTED_SUCCESS"),
    }
}
