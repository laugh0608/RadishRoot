//! BSP 异常与唯一外部 IRQ0；ISR 无分配 / 锁 / 串口 / 文件访问。
use core::{
    fmt::{self, Write},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};
use x86_64::{
    VirtAddr,
    instructions::{
        interrupts,
        port::Port,
        segmentation::{CS, DS, ES, SS, Segment},
        tables::load_tss,
    },
    structures::{
        gdt::{Descriptor, GlobalDescriptorTable},
        idt::{InterruptDescriptorTable, InterruptStackFrame},
        tss::TaskStateSegment,
    },
};
static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();
static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::new();
static mut TSS: TaskStateSegment = TaskStateSegment::new();
#[repr(C, align(4096))]
struct Stack([u8; 5 * 4096]);
static mut FAULT_STACK: Stack = Stack([0; 5 * 4096]);
static PANICKING: AtomicBool = AtomicBool::new(false);
static TICKS: AtomicU64 = AtomicU64::new(0);
static PENDING: AtomicBool = AtomicBool::new(false);
static TICKS_SATURATED: AtomicBool = AtomicBool::new(false);
pub const FAULT_ADDRESS: u64 = 0xffffd00000000000;
struct Emergency {
    remaining: usize,
}
impl Write for Emergency {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for byte in text.bytes() {
            if self.remaining == 0 {
                return Err(fmt::Error);
            }
            self.remaining -= 1;
            let mut sent = false;
            for _ in 0..100_000 {
                // SAFETY: 固定 COM1；致命路径 IF=0 永不返回，接管已暂停上下文，不持有 UART 引用。
                unsafe {
                    if Port::<u8>::new(0x3fd).read() & 0x20 != 0 {
                        Port::<u8>::new(0x3f8).write(byte);
                        sent = true;
                        break;
                    }
                }
                core::hint::spin_loop();
            }
            if !sent {
                return Err(fmt::Error);
            }
        }
        Ok(())
    }
}
pub fn fatal(args: fmt::Arguments<'_>) -> ! {
    interrupts::disable();
    let mut writer = Emergency { remaining: 2048 };
    if PANICKING.swap(true, Ordering::Relaxed) {
        let _ = writer.write_str("\nRR:T06:REENTRANT\n");
    } else {
        let _ = writer.write_fmt(args);
    }
    crate::halt()
}
fn exception(frame: InterruptStackFrame, vector: u8, error: Option<u64>) {
    let code = match vector {
        14 => "PAGE_FAULT",
        8 => "DOUBLE_FAULT",
        13 => "GENERAL_PROTECTION",
        6 => "INVALID_OPCODE",
        _ => "CPU_EXCEPTION",
    };
    let cr2 = x86_64::registers::control::Cr2::read_raw();
    let rsp: u64;
    // SAFETY: 只读当前栈指针，用于验证 IST；不改变寄存器 / 内存。
    unsafe {
        core::arch::asm!("mov {}, rsp", out(reg) rsp, options(nomem, nostack, preserves_flags));
    }
    let base = core::ptr::addr_of!(FAULT_STACK) as u64;
    fatal(format_args!(
        "\nRR:T06:EXCEPTION vector={vector} code={code} error={} has_error={} rip={:#x} rsp={:#x} rflags={:#x} cr2={cr2:#x} handler_rsp={rsp:#x} ist_start={:#x} ist_end={:#x}\nRR:T06:FAIL code={code} stage=exception\n",
        error.unwrap_or(0),
        u8::from(error.is_some()),
        frame.instruction_pointer.as_u64(),
        frame.stack_pointer.as_u64(),
        frame.cpu_flags.bits(),
        base + 4096,
        base + 5 * 4096
    ));
}
pub fn early() {
    interrupts::disable();
    // SAFETY: 唯一 BSP IF=0，首次构造后不再持有可变引用；早期使用引导器当前 CS。
    unsafe {
        let idt = &mut *core::ptr::addr_of_mut!(IDT);
        x86_64::set_general_handler!(idt, exception);
        idt.load_unsafe();
    }
}
extern "x86-interrupt" fn timer(_: InterruptStackFrame) {
    let ticks = TICKS.load(Ordering::Relaxed);
    if let Some(next) = ticks.checked_add(1) {
        TICKS.store(next, Ordering::Relaxed);
    } else {
        TICKS_SATURATED.store(true, Ordering::Relaxed);
    }
    PENDING.store(true, Ordering::Release);
    // SAFETY: 唯一未屏蔽外部中断 IRQ0 来自主 PIC，写 EOI；不访问其他设备。
    unsafe {
        Port::<u8>::new(0x20).write(0x20);
    }
}
extern "x86-interrupt" fn keyboard(_: InterruptStackFrame) {
    crate::ps2::irq(false);
}
extern "x86-interrupt" fn mouse(_: InterruptStackFrame) {
    crate::ps2::irq(true);
}
pub fn wake() {
    PENDING.store(true, Ordering::Release);
}
pub fn input_enabled(enabled: bool) {
    interrupts::without_interrupts(|| {
        out(0x21, if enabled { 0xf8 } else { 0xfe });
        out(0xa1, if enabled { 0xef } else { 0xff });
    });
}
fn out(port: u16, value: u8) {
    // SAFETY: 调用点仅 PIC/PIT 初始化，IF=0、固定 q35 端口；0x80 为传统 IO delay。
    unsafe {
        Port::<u8>::new(port).write(value);
        Port::<u8>::new(0x80).write(0);
    }
}
pub fn initialize() -> Result<(u64, u64, u64, u64), &'static str> {
    interrupts::disable();
    let base = core::ptr::addr_of!(FAULT_STACK) as u64;
    // SAFETY: 静态栈按页对齐且首个4096字节未使用；之后只作为 guard，无其他引用。
    unsafe {
        crate::platform_memory::guard_stack(base)?;
    }
    // SAFETY: BSP/IF=0；GDT/TSS 静态终生保存，TSS 写完才发布 descriptor，之后不再修改。
    unsafe {
        let tss = &mut *core::ptr::addr_of_mut!(TSS);
        tss.interrupt_stack_table[0] = VirtAddr::new(base + 5 * 4096);
        let gdt = &mut *core::ptr::addr_of_mut!(GDT);
        let code = gdt.append(Descriptor::kernel_code_segment());
        let data = gdt.append(Descriptor::kernel_data_segment());
        let tss_selector = gdt.append(Descriptor::tss_segment(&*core::ptr::addr_of!(TSS)));
        gdt.load_unsafe();
        CS::set_reg(code);
        SS::set_reg(data);
        DS::set_reg(data);
        ES::set_reg(data);
        load_tss(tss_selector);
        let idt = &mut *core::ptr::addr_of_mut!(IDT);
        x86_64::set_general_handler!(idt, exception);
        idt.double_fault
            .set_handler_fn(double_fault)
            .set_stack_index(0);
        idt[32].set_handler_fn(timer);
        idt[33].set_handler_fn(keyboard);
        idt[44].set_handler_fn(mouse);
        idt.load_unsafe();
    }
    // 固定单核 PIC 模式不能继承固件的 LAPIC/LINT0 路由；显式关闭本地 APIC。
    // x86_64 CPUID；随后只在 CPU 声明 MSR / APIC 能力时访问 IA32_APIC_BASE。
    let features = core::arch::x86_64::__cpuid(1).edx;
    if features & ((1 << 5) | (1 << 9)) != (1 << 5) | (1 << 9) {
        return Err("IRQ_CPU_UNSUPPORTED");
    }
    let (apic_before, apic_after) = unsafe {
        let mut base = x86_64::registers::model_specific::Msr::new(0x1b);
        let before = base.read();
        if before & (1 << 10) != 0 {
            return Err("IRQ_X2APIC_UNSUPPORTED");
        }
        // SAFETY: CPL0 / IF=0，单 BSP、无 APIC 用户；只清 enable 位，保留基址及 BSP 位。
        base.write(before & !(1 << 11));
        (before, base.read())
    };
    if apic_after != apic_before & !(1 << 11) {
        return Err("IRQ_APIC_DISABLE");
    }
    out(0x21, 0xff);
    out(0xa1, 0xff);
    out(0x20, 0x11);
    out(0xa0, 0x11);
    out(0x21, 0x20);
    out(0xa1, 0x28);
    out(0x21, 4);
    out(0xa1, 2);
    out(0x21, 1);
    out(0xa1, 1);
    out(0x21, 0xfe);
    out(0xa1, 0xff);
    // mode 2, lo/hi，1193182 / 11932 ≈100 Hz；仅唤醒，不作为时钟来源。
    out(0x43, 0x34);
    out(0x40, (11932u16 & 255) as u8);
    out(0x40, (11932u16 >> 8) as u8);
    Ok((base + 4096, base + 5 * 4096, apic_before, apic_after))
}
pub fn diagnostic() -> (u8, u8, u8) {
    interrupts::without_interrupts(|| {
        // SAFETY: 主流程 IF=0 读取 PIC 掩码 / IRR / ISR；不应答或屏蔽中断，仅诊断失败。
        unsafe {
            let mask = Port::<u8>::new(0x21).read();
            Port::<u8>::new(0x20).write(0x0a);
            let irr = Port::<u8>::new(0x20).read();
            Port::<u8>::new(0x20).write(0x0b);
            let isr = Port::<u8>::new(0x20).read();
            (mask, irr, isr)
        }
    })
}
extern "x86-interrupt" fn double_fault(frame: InterruptStackFrame, error: u64) -> ! {
    exception(frame, 8, Some(error));
    crate::halt()
}
pub fn ticks() -> (u64, bool) {
    (
        TICKS.load(Ordering::Relaxed),
        TICKS_SATURATED.load(Ordering::Relaxed),
    )
}
pub fn take_wake() -> bool {
    PENDING.swap(false, Ordering::AcqRel)
}
pub fn idle(work_pending: bool) {
    interrupts::disable();
    if work_pending || PENDING.load(Ordering::Acquire) {
        interrupts::enable();
    } else {
        interrupts::enable_and_hlt();
    }
}
#[unsafe(no_mangle)]
#[inline(never)]
pub extern "C" fn t06_fault_probe() {
    // SAFETY: boot-test 唯一调用，故意访问已确认未映射测试页，异常 handler 永不返回；无 Rust 引用。
    unsafe {
        core::arch::asm!("mov rax, [rdi]", in("rdi") FAULT_ADDRESS, out("rax") _, options(nostack));
    }
}
pub fn inject_fault(double: bool) -> ! {
    if !cfg!(feature = "boot-test") {
        panic!("fault injection requires boot-test");
    }
    assert!(crate::platform_memory::unmapped(FAULT_ADDRESS));
    interrupts::disable();
    if double {
        // SAFETY: boot-test / IF=0 独占改变 page-fault gate，随后故意触发异常交付失败；不返回。
        unsafe {
            (&mut *core::ptr::addr_of_mut!(IDT))
                .page_fault
                .set_handler_fn(page_fault_unused)
                .set_present(false);
        }
    }
    t06_fault_probe();
    fatal(format_args!(
        "RR:T06:FAIL code=FAULT_NOT_TRIGGERED stage=inject\n"
    ))
}
extern "x86-interrupt" fn page_fault_unused(
    frame: InterruptStackFrame,
    error: x86_64::structures::idt::PageFaultErrorCode,
) {
    exception(frame, 14, Some(error.bits()));
}
