use crate::serial::Serial;
use core::{fmt::Write, mem::align_of};
use limine::{BaseRevision, RequestsEndMarker, RequestsStartMarker, request::*};
use radishroot_kernel::boot_contract::{self, BootError, Framebuffer, MAX_REGIONS, Region};

#[used]
#[unsafe(link_section = ".limine_requests_start")]
static START: RequestsStartMarker = RequestsStartMarker::new();
#[used]
#[unsafe(link_section = ".limine_requests")]
static BASE: BaseRevision = BaseRevision::with_revision(6);
macro_rules! request {
    ($name:ident, $ty:ty, $value:expr) => {
        #[used]
        #[unsafe(link_section = ".limine_requests")]
        static $name: $ty = $value;
    };
}
request!(MAP, MemmapRequest, MemmapRequest::new());
request!(FB, FramebufferRequest, FramebufferRequest::new());
request!(HHDM, HhdmRequest, HhdmRequest::new());
request!(RSDP, RsdpRequest, RsdpRequest::new());
request!(
    ADDRESS,
    ExecutableAddressRequest,
    ExecutableAddressRequest::new()
);
request!(STACK, StackSizeRequest, StackSizeRequest::new(64 * 1024));
request!(
    CMDLINE,
    ExecutableCmdlineRequest,
    ExecutableCmdlineRequest::new()
);
#[used]
#[unsafe(link_section = ".limine_requests_end")]
static END: RequestsEndMarker = RequestsEndMarker::new();

// 唯一布局来自 Limine revision 0 的 counted response 公共前缀：revision/count/pointer。
// 0.6.5 的便利方法在校验 count 前构造 slice，因此在边界处先读取这个前缀。
#[repr(C)]
struct Counted {
    revision: u64,
    count: u64,
    pointers: *const *const (),
}
unsafe fn pointers<R, T>(response: &Response<R>, maximum: usize) -> Result<&[*const T], BootError> {
    // SAFETY: 调用点仅限 Memmap / Framebuffer 的 repr(C) response，均至少 24 字节；
    // 已核对固定协议和 crate 布局。响应由引导器映射，T02 不卸载或回收这些内存。
    let header = unsafe { &*(response as *const Response<R>).cast::<Counted>() };
    if header.count == 0
        || header.count > maximum as u64
        || header.pointers.is_null()
        || !(header.pointers as usize).is_multiple_of(align_of::<*const T>())
    {
        return Err(BootError::MissingResponse);
    }
    // SAFETY: count 已有界且数组地址对齐；指针数组的可读映射由受信引导器保证。
    // 先构造 raw pointers slice，再逐项检查，不把未经检查的空指针变为引用。
    Ok(unsafe { core::slice::from_raw_parts(header.pointers.cast(), header.count as usize) })
}
unsafe fn object<T>(pointer: *const T) -> Result<&'static T, BootError> {
    if pointer.is_null() || !(pointer as usize).is_multiple_of(align_of::<T>()) {
        return Err(BootError::MissingResponse);
    }
    // SAFETY: 指针来自协议有效响应；以上仅补空值 / 对齐校验，映射与生命周期依赖
    // 受信引导交接。没有声称可以安全解析恶意引导器任意物理内存，T02 不回收它。
    Ok(unsafe { &*pointer })
}
fn emit(serial: &mut Serial, args: core::fmt::Arguments<'_>) {
    if serial.write_fmt(args).is_err() {
        crate::halt();
    }
}
pub struct Facts {
    pub regions: [Region; MAX_REGIONS],
    pub count: usize,
    pub hhdm: u64,
    pub mode: &'static str,
    pub rsdp: Option<u64>,
    pub framebuffer: Framebuffer,
    pub framebuffer_address: u64,
}
pub fn run(serial: &mut Serial) -> Result<Facts, BootError> {
    emit(serial, format_args!("\nRR:T02:ENTER\n"));
    let mode = if cfg!(feature = "boot-test") {
        CMDLINE.response().map(|r| r.cmdline()).unwrap_or("")
    } else {
        ""
    };
    if !mode.is_empty()
        && !mode.starts_with("t03-")
        && !mode.starts_with("t04-")
        && !mode.starts_with("t05-")
        && !mode.starts_with("t06-")
        && !mode.starts_with("t07-")
        && !mode.starts_with("t08-")
        && !mode.starts_with("t09-")
    {
        emit(serial, format_args!("RR:T02:INJECT case={mode}\n"));
    }
    boot_contract::protocol(
        BASE.is_supported() && mode != "unsupported-protocol",
        BASE.actual_revision(),
        mode != "missing-response",
    )?;
    let map = MAP.response().ok_or(BootError::MissingResponse)?;
    let fb = FB.response().ok_or(BootError::MissingResponse)?;
    let hhdm = HHDM.response().ok_or(BootError::MissingResponse)?;
    let address = ADDRESS.response().ok_or(BootError::MissingResponse)?;
    STACK.response().ok_or(BootError::MissingResponse)?;
    // SAFETY: 仅上述两类已定义 counted responses；请求为 revision 0。
    let entries = unsafe { pointers::<_, limine::memmap::Entry>(map, MAX_REGIONS)? };
    let framebuffers = unsafe { pointers::<_, limine::framebuffer::Framebuffer>(fb, 16)? };
    let mut regions = [Region::default(); MAX_REGIONS];
    for (i, pointer) in entries.iter().enumerate() {
        // SAFETY: 由受信引导器响应数组给出的有效内存图条目，生命周期不被改变。
        let entry = unsafe { object(*pointer)? };
        regions[i] = Region {
            base: entry.base,
            length: entry.length,
            kind: entry.type_,
        };
    }
    // SAFETY: framebuffer response 同上；不解引用像素 / EDID / 模式列表。
    let fb = unsafe { object(framebuffers[0])? };
    let physical = (fb.address() as u64)
        .checked_sub(hhdm.offset)
        .ok_or(BootError::FramebufferRange)?;
    unsafe extern "C" {
        static __kernel_end: u8;
    }
    let virtual_end = core::ptr::addr_of!(__kernel_end) as u64;
    let kernel_length = virtual_end
        .checked_sub(address.virtual_base)
        .ok_or(BootError::KernelRange)?;
    if address.virtual_base != 0xffffffff80000000 {
        return Err(BootError::KernelRange);
    }
    let mut framebuffer = Framebuffer {
        physical,
        width: fb.width,
        height: fb.height,
        pitch: fb.pitch,
        bpp: fb.bpp,
        model: fb.memory_model,
        masks: [
            (fb.red_mask_size, fb.red_mask_shift),
            (fb.green_mask_size, fb.green_mask_shift),
            (fb.blue_mask_size, fb.blue_mask_shift),
        ],
    };
    if mode == "invalid-framebuffer" {
        framebuffer.pitch = 0;
    }
    let usable = boot_contract::validate(
        &regions[..entries.len()],
        address.physical_base,
        kernel_length,
        framebuffer,
    )?;
    for (i, region) in regions[..entries.len()].iter().enumerate() {
        emit(
            serial,
            format_args!(
                "RR:T02:MEM index={i} base={:#x} length={:#x} kind={}\n",
                region.base, region.length, region.kind
            ),
        );
    }
    emit(
        serial,
        format_args!(
            "RR:T02:KERNEL physical={:#x} length={:#x} virtual={:#x}\n",
            address.physical_base, kernel_length, address.virtual_base
        ),
    );
    emit(
        serial,
        format_args!(
            "RR:T02:FB physical={physical:#x} width={} height={} pitch={} bpp={}\n",
            fb.width, fb.height, fb.pitch, fb.bpp
        ),
    );
    emit(
        serial,
        format_args!(
            "RR:T02:MEMORY entries={} usable={usable} reclaim=none\n",
            entries.len()
        ),
    );
    if mode == "panic" {
        panic!("T02 explicit panic injection");
    }
    if ![
        "",
        "panic",
        "unsupported-protocol",
        "missing-response",
        "invalid-framebuffer",
        "t03-invalid-range",
        "t03-bad-capability",
        "t03-dma-timeout",
        "t04-write",
        "t04-reopen",
        "t04-primary-gpt",
        "t04-both-gpt",
        "t04-conflict-gpt",
        "t04-timeout",
        "t04-no-flush",
        "t05-write",
        "t05-reopen",
        "t05-readonly",
        "t05-file-sync",
        "t05-volume-sync",
        "t05-device-flush",
        "t05-bad-fat",
        "t06-page-fault",
        "t06-double-fault",
        "t06-alloc-failure",
        "t06-overflow",
        "t06-clock-stalled",
        "t06-invalid-input",
        "t06-timeout",
        "t06-no-hpet",
        "t07-resolution",
        "t07-surface-allocation",
        "t07-surface-description",
        "t07-ps2-timeout",
        "t07-corrupt-input",
        "t07-input-overflow",
        "t07-clipping",
        "t07-redraw",
        "t08-write",
        "t08-reopen",
        "t08-readonly",
        "t08-file-sync",
        "t08-volume-sync",
        "t08-device-flush",
        "t08-bad-fat",
        "t09-write",
        "t09-reopen",
        "t09-readonly",
        "t09-file-sync",
        "t09-volume-sync",
        "t09-device-flush",
        "t09-bad-fat",
    ]
    .contains(&mode)
    {
        return Err(BootError::Protocol);
    }
    emit(serial, format_args!("RR:T02:READY revision=6\n"));
    Ok(Facts {
        regions,
        count: entries.len(),
        hhdm: hhdm.offset,
        mode,
        rsdp: RSDP.response().map(|r| r.address as u64),
        framebuffer,
        framebuffer_address: fb.address() as u64,
    })
}
