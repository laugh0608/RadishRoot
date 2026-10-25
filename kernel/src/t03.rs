use crate::{
    boot::Facts,
    heap, pci,
    platform_memory::{self, VirtioHal},
    serial::Serial,
};
use core::{
    alloc::{GlobalAlloc, Layout},
    fmt::Write,
    ptr::NonNull,
};
use virtio_drivers::{BufferDirection, Hal};
fn emit(serial: &mut Serial, args: core::fmt::Arguments<'_>) -> Result<(), &'static str> {
    serial.write_fmt(args).map_err(|_| "SERIAL_UNAVAILABLE")
}
pub fn run(serial: &mut Serial, facts: &Facts) -> Result<pci::Device, &'static str> {
    emit(serial, format_args!("RR:T03:ENTER\n"))?;
    let mode = facts.mode;
    if mode.starts_with("t03-") {
        emit(serial, format_args!("RR:T03:INJECT case={mode}\n"))?;
    }
    let (pages, heap_base) =
        platform_memory::initialize(&facts.regions[..facts.count], facts.hhdm)?;
    let page = platform_memory::page_check()?;
    let layout = Layout::from_size_align(8192, 4096).map_err(|_| "HEAP_LAYOUT")?;
    let before = heap::used();
    // SAFETY: layout 非零；分配成功后仅本函数访问，最后以同一 layout 释放。
    let p = unsafe { heap::HEAP.alloc(layout) };
    if p.is_null() || !(p as usize).is_multiple_of(4096) {
        return Err("HEAP_ALLOCATION");
    }
    unsafe {
        p.write_bytes(0x5a, 8192);
        if (0..8192).any(|i| p.add(i).read() != 0x5a) {
            return Err("HEAP_READBACK");
        }
        heap::HEAP.dealloc(p, layout);
    }
    if heap::used() != before {
        return Err("HEAP_ACCOUNTING");
    }
    emit(
        serial,
        format_args!(
            "RR:T03:MEM pages={pages} page_size=4096 heap_physical={heap_base:#x} heap_bytes=1048576 test_physical={page:#x} readback=pass reclaim=none\n"
        ),
    )?;
    if mode == "t03-invalid-range" {
        return Err(platform_memory::reject_invalid_allocation().unwrap_err());
    }
    let device = pci::discover(&facts.regions[..facts.count], mode == "t03-bad-capability")?;
    for (i, bar) in device
        .bars
        .iter()
        .enumerate()
        .filter(|(_, bar)| bar.length != 0)
    {
        emit(
            serial,
            format_args!(
                "RR:T03:BAR index={i} base={:#x} length={:#x} cache=uc\n",
                bar.base, bar.length
            ),
        )?;
    }
    emit(
        serial,
        format_args!(
            "RR:T03:PCI bus={} device={} function={} vendor=0x1af4 id=0x1042 capabilities={} features={:#x} queue_size={} bus_master={}\n",
            device.bdf.bus,
            device.bdf.device,
            device.bdf.function,
            device.capabilities,
            device.features,
            device.queue_size,
            u8::from(device.bus_master)
        ),
    )?;
    let (physical, pointer) = VirtioHal::dma_alloc(2, BufferDirection::Both);
    // SAFETY: HAL 刚分配的独占 RAM，设备未配置队列；只用原始指针读写，释放前结束访问。
    unsafe {
        for i in 0..8192 {
            if pointer.as_ptr().add(i).read() != 0 {
                return Err("DMA_NOT_ZERO");
            }
        }
    }
    let buffer = NonNull::slice_from_raw_parts(pointer, 512);
    unsafe {
        pointer.as_ptr().write_bytes(0x37, 512);
    }
    // SAFETY: 原缓冲来自当前 HAL，长度在已分配块内，share 到 unshare 期间仅本测试持有所有权。
    let bounce = unsafe { VirtioHal::share(buffer, BufferDirection::Both) };
    if mode == "t03-dma-timeout" {
        platform_memory::dma_transition(bounce, true)?;
        platform_memory::dma_transition(bounce, false)?;
        let refused = platform_memory::reject_busy_unshare(bounce);
        // SAFETY: 用真实原参数尝试释放，HAL 必须因仍被共享而拒绝，不实际释放。
        let allocation_refused = unsafe { VirtioHal::dma_dealloc(physical, pointer, 2) } != 0;
        emit(
            serial,
            format_args!(
                "RR:T03:QUARANTINE physical={physical:#x} bounce={bounce:#x} release_rejected={} allocation_rejected={} reused=0\n",
                u8::from(refused.is_err()),
                u8::from(allocation_refused)
            ),
        )?;
        return if refused.is_err() && allocation_refused {
            Err("DMA_IN_FLIGHT")
        } else {
            Err("TEST_UNEXPECTED_SUCCESS")
        };
    }
    // SAFETY: 无请求提交且 bus master 禁用；仅本调用持有原缓冲。先清空原缓冲，
    // 再取消 Prepared 的 share 并 copy-back，以独立确认 bounce 的内容确实回传。
    unsafe {
        pointer.as_ptr().write_bytes(0, 512);
        VirtioHal::unshare(bounce, buffer, BufferDirection::Both);
        for i in 0..512 {
            if pointer.as_ptr().add(i).read() != 0x37 {
                return Err("DMA_READBACK");
            }
        }
        if VirtioHal::dma_dealloc(physical, pointer, 2) != 0 {
            return Err("DMA_DEALLOC");
        }
    }
    emit(
        serial,
        format_args!(
            "RR:T03:DMA physical={physical:#x} bounce={bounce:#x} pages=2 zeroed=1 shared=512 cancelled_before_submit=1 released=1\n"
        ),
    )?;
    emit(serial, format_args!("RR:T03:READY storage_io=none\n"))?;
    Ok(device)
}
