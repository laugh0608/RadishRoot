//! T02 引导事实校验。没有指针解引用、分配器或硬件访问，供 guest 与宿主测试共用。
pub const MAX_REGIONS: usize = 256;
#[derive(Clone, Copy, Debug, Default)]
pub struct Region {
    pub base: u64,
    pub length: u64,
    pub kind: u64,
}
#[derive(Clone, Copy, Debug)]
pub struct Framebuffer {
    pub physical: u64,
    pub width: u64,
    pub height: u64,
    pub pitch: u64,
    pub bpp: u16,
    pub model: u8,
    pub masks: [(u8, u8); 3],
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BootError {
    Protocol,
    MissingResponse,
    MemoryCount,
    MemoryRange,
    MemoryOverlap,
    NoUsableMemory,
    KernelRange,
    FramebufferGeometry,
    FramebufferFormat,
    FramebufferRange,
}
impl BootError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Protocol => "BOOT_PROTOCOL",
            Self::MissingResponse => "BOOT_RESPONSE_MISSING",
            Self::MemoryCount => "BOOT_MEMORY_COUNT",
            Self::MemoryRange => "BOOT_MEMORY_RANGE",
            Self::MemoryOverlap => "BOOT_MEMORY_OVERLAP",
            Self::NoUsableMemory => "BOOT_NO_USABLE_MEMORY",
            Self::KernelRange => "BOOT_KERNEL_RANGE",
            Self::FramebufferGeometry => "BOOT_FB_GEOMETRY",
            Self::FramebufferFormat => "BOOT_FB_FORMAT",
            Self::FramebufferRange => "BOOT_FB_RANGE",
        }
    }
}
pub fn protocol(supported: bool, actual: Option<u64>, present: bool) -> Result<(), BootError> {
    if !supported || actual != Some(6) {
        return Err(BootError::Protocol);
    }
    if !present {
        return Err(BootError::MissingResponse);
    }
    Ok(())
}
pub fn memory(regions: &[Region]) -> Result<u64, BootError> {
    if regions.is_empty() || regions.len() > MAX_REGIONS {
        return Err(BootError::MemoryCount);
    }
    let mut previous = 0;
    let mut usable = 0u64;
    for region in regions {
        let end = region
            .base
            .checked_add(region.length)
            .filter(|_| region.length != 0)
            .ok_or(BootError::MemoryRange)?;
        if region.base < previous {
            return Err(BootError::MemoryOverlap);
        }
        previous = end;
        // Unknown types remain reserved; never count them as usable.
        if region.kind == 0 {
            usable = usable
                .checked_add(region.length)
                .ok_or(BootError::MemoryRange)?;
        }
    }
    if usable == 0 {
        return Err(BootError::NoUsableMemory);
    }
    Ok(usable)
}
pub fn covered(regions: &[Region], base: u64, length: u64, kind: u64) -> bool {
    let Some(end) = base.checked_add(length).filter(|_| length != 0) else {
        return false;
    };
    regions.iter().any(|r| {
        r.kind == kind
            && r.base <= base
            && r.base
                .checked_add(r.length)
                .is_some_and(|limit| end <= limit)
    })
}
pub fn validate(
    regions: &[Region],
    kernel_base: u64,
    kernel_length: u64,
    fb: Framebuffer,
) -> Result<u64, BootError> {
    let usable = memory(regions)?;
    if !covered(regions, kernel_base, kernel_length, 6) {
        return Err(BootError::KernelRange);
    }
    if fb.width == 0
        || fb.height == 0
        || fb.width > 8192
        || fb.height > 8192
        || fb.width.checked_mul(4).is_none_or(|row| fb.pitch < row)
    {
        return Err(BootError::FramebufferGeometry);
    }
    if fb.bpp != 32 || fb.model != 1 {
        return Err(BootError::FramebufferFormat);
    }
    let mut bits = 0u32;
    for (size, shift) in fb.masks {
        if size != 8 || shift > 24 {
            return Err(BootError::FramebufferFormat);
        }
        let mask = 0xffu32 << shift;
        if bits & mask != 0 {
            return Err(BootError::FramebufferFormat);
        }
        bits |= mask;
    }
    let length = fb
        .pitch
        .checked_mul(fb.height)
        .filter(|n| *n <= 256 * 1024 * 1024)
        .ok_or(BootError::FramebufferGeometry)?;
    if !covered(regions, fb.physical, length, 7) {
        return Err(BootError::FramebufferRange);
    }
    Ok(usable)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn regions() -> [Region; 4] {
        [
            Region {
                base: 0,
                length: 4096,
                kind: 1,
            },
            Region {
                base: 4096,
                length: 4096,
                kind: 0,
            },
            Region {
                base: 8192,
                length: 4096,
                kind: 6,
            },
            Region {
                base: 0xe0000000,
                length: 4 * 1024 * 768,
                kind: 7,
            },
        ]
    }
    fn fb() -> Framebuffer {
        Framebuffer {
            physical: 0xe0000000,
            width: 1024,
            height: 768,
            pitch: 4096,
            bpp: 32,
            model: 1,
            masks: [(8, 16), (8, 8), (8, 0)],
        }
    }
    #[test]
    fn accepts_facts_and_counts_only_usable() {
        assert_eq!(validate(&regions(), 8192, 100, fb()), Ok(4096));
        let mut r = regions();
        r[0].kind = 99;
        assert_eq!(validate(&r, 8192, 100, fb()), Ok(4096));
    }
    #[test]
    fn rejects_protocol_and_missing_response() {
        assert_eq!(protocol(false, Some(6), true), Err(BootError::Protocol));
        assert_eq!(protocol(true, Some(5), true), Err(BootError::Protocol));
        assert_eq!(protocol(true, None, true), Err(BootError::Protocol));
        assert_eq!(
            protocol(true, Some(6), false),
            Err(BootError::MissingResponse)
        );
    }
    #[test]
    fn rejects_map_bounds_and_overlap() {
        assert_eq!(memory(&[]), Err(BootError::MemoryCount));
        assert_eq!(
            memory(&[Region::default(); 257]),
            Err(BootError::MemoryCount)
        );
        let mut r = regions();
        r[0].base = u64::MAX;
        assert_eq!(memory(&r), Err(BootError::MemoryRange));
        let mut r = regions();
        r[1].base = 2048;
        assert_eq!(memory(&r), Err(BootError::MemoryOverlap));
        let mut r = regions();
        r[1].kind = 1;
        assert_eq!(memory(&r), Err(BootError::NoUsableMemory));
    }
    #[test]
    fn rejects_reserved_overlap_geometry_and_format() {
        assert_eq!(
            validate(&regions(), 0, 100, fb()),
            Err(BootError::KernelRange)
        );
        let mut f = fb();
        f.pitch = 0;
        assert_eq!(
            validate(&regions(), 8192, 100, f),
            Err(BootError::FramebufferGeometry)
        );
        let mut f = fb();
        f.masks[0] = (8, 0);
        assert_eq!(
            validate(&regions(), 8192, 100, f),
            Err(BootError::FramebufferFormat)
        );
        let mut f = fb();
        f.physical = 4096;
        assert_eq!(
            validate(&regions(), 8192, 100, f),
            Err(BootError::FramebufferRange)
        );
        let mut f = fb();
        f.pitch = u64::MAX;
        assert_eq!(
            validate(&regions(), 8192, 100, f),
            Err(BootError::FramebufferGeometry)
        );
    }
}
