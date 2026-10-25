//! 固定平台 HPET：ACPI 只读发现、独占 UC 映射，IRQ 不读取此模块。
use crate::{boot::Facts, exclusive::Exclusive, platform_memory};
use radishroot_kernel::{
    acpi,
    runtime_contract::{Clock, TimeError},
};
struct State {
    base: usize,
    clock: Clock,
}
static STATE: Exclusive<Option<State>> = Exclusive::new(None);
fn bytes(facts: &Facts, physical: u64, length: usize) -> Result<&[u8], &'static str> {
    if !acpi::mapped_table_range(physical, length, &facts.regions[..facts.count]) {
        return Err("ACPI_RANGE");
    }
    let virtual_address = facts.hhdm.checked_add(physical).ok_or("ACPI_RANGE")?;
    x86_64::VirtAddr::try_new(virtual_address).map_err(|_| "ACPI_RANGE")?;
    x86_64::VirtAddr::try_new(
        virtual_address
            .checked_add(length as u64 - 1)
            .ok_or("ACPI_RANGE")?,
    )
    .map_err(|_| "ACPI_RANGE")?;
    // SAFETY: Limine base revision 6 保证这些 ACPI 区间的 HHDM 映射；长度先验证且只读，内存不回收。
    Ok(unsafe { core::slice::from_raw_parts(virtual_address as *const u8, length) })
}
fn table(facts: &Facts, physical: u64) -> Result<&[u8], &'static str> {
    let header = bytes(facts, physical, 36)?;
    let length = acpi::table_length(header).map_err(|_| "ACPI_LENGTH")?;
    bytes(facts, physical, length)
}
fn discover(facts: &Facts) -> Result<u64, &'static str> {
    let rsdp = facts
        .rsdp
        .ok_or("ACPI_RSDP_MISSING")?
        .checked_sub(facts.hhdm)
        .ok_or("ACPI_RANGE")?;
    let header = bytes(facts, rsdp, 36)?;
    let length = u32::from_le_bytes(header[20..24].try_into().unwrap()) as usize;
    let root = acpi::rsdp(bytes(facts, rsdp, length)?).map_err(|_| "ACPI_RSDP")?;
    let entries = acpi::xsdt(table(facts, root)?).map_err(|_| "ACPI_XSDT")?;
    let mut found = None;
    for entry in entries.as_chunks::<8>().0 {
        let address = u64::from_le_bytes(*entry);
        let header = bytes(facts, address, 36)?;
        if &header[..4] == b"HPET" {
            let base = acpi::hpet(table(facts, address)?).map_err(|_| "ACPI_HPET")?;
            if found.replace(base).is_some() {
                return Err("HPET_MULTIPLE");
            }
        }
    }
    found.ok_or("HPET_MISSING")
}
fn read(base: usize, offset: usize) -> u32 {
    // SAFETY: base 仅来自本模块保存的已验证独占 HPET 映射，调用点 offset 为标准寄存器且4字节对齐。
    unsafe { ((base + offset) as *const u32).read_volatile() }
}
fn write(base: usize, offset: usize, value: u32) {
    // SAFETY: 同上，只写配置寄存器；初始化 / 测试均在 STATE 锁内或发布前，IRQ 不访问 HPET。
    unsafe {
        ((base + offset) as *mut u32).write_volatile(value);
    }
}
fn counter(base: usize) -> Result<u64, TimeError> {
    for _ in 0..8 {
        let hi = read(base, 0xf4);
        let lo = read(base, 0xf0);
        if hi == read(base, 0xf4) {
            return Ok(u64::from(hi) << 32 | u64::from(lo));
        }
    }
    Err(TimeError::Inconsistent)
}
pub fn initialize(facts: &Facts) -> Result<(u64, u64), &'static str> {
    let physical = discover(facts)?;
    let base = platform_memory::map_hpet(physical, &facts.regions[..facts.count])?;
    let caps = read(base, 0);
    let period = u64::from(read(base, 4));
    if caps & (1 << 13) == 0 {
        return Err("HPET_COUNTER_WIDTH");
    }
    let config = read(base, 0x10);
    write(base, 0x10, config & !3);
    for i in 0..=((caps >> 8) & 31) as usize {
        let offset = 0x100 + 0x20 * i;
        write(base, offset, read(base, offset) & !(1 << 2));
    }
    // 不重置主计数，不启用 legacy replacement 或 comparator IRQ。
    write(base, 0x10, (config & !3) | 1);
    let mut clock =
        Clock::new(period, counter(base).map_err(TimeError::code)?).map_err(TimeError::code)?;
    let mut progressed = false;
    for _ in 0..100_000 {
        if clock
            .observe(counter(base).map_err(TimeError::code)?)
            .map_err(TimeError::code)?
            > 0
        {
            progressed = true;
            break;
        }
        core::hint::spin_loop();
    }
    if !progressed {
        return Err("CLOCK_STALLED");
    }
    let mut state = STATE.lock();
    if state.is_some() {
        return Err("CLOCK_REINITIALIZE");
    }
    *state = Some(State { base, clock });
    Ok((physical, period))
}
pub fn now() -> Result<u64, TimeError> {
    let mut state = STATE.lock();
    let state = state.as_mut().ok_or(TimeError::Unavailable)?;
    state.clock.observe(counter(state.base)?)
}
pub fn inject_stop() -> (u64, u64, u32) {
    if !cfg!(feature = "boot-test") {
        panic!("fault injection requires boot-test");
    }
    let state = STATE.lock();
    let s = state.as_ref().expect("clock initialized");
    write(s.base, 0x10, read(s.base, 0x10) & !1);
    let before = counter(s.base).expect("stopped counter read");
    for _ in 0..100 {
        core::hint::spin_loop();
    }
    (
        before,
        counter(s.base).expect("stopped counter read"),
        read(s.base, 0x10),
    )
}
