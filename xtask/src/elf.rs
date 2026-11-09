use crate::error::{Error, Result};
use std::{fs::File, io::Read, path::Path};

const LIMIT: u64 = 64 * 1024 * 1024;
#[derive(Debug)]
pub struct Layout {
    pub entry: u64,
    pub loads: usize,
}
fn bad(kind: &'static str) -> Error {
    Error::new(kind, "invalid ELF64 kernel")
}
fn range(offset: u64, size: u64, len: usize) -> Result<std::ops::Range<usize>> {
    let end = offset
        .checked_add(size)
        .ok_or_else(|| bad("elf-overflow"))?;
    if end > len as u64 {
        return Err(bad("elf-bounds"));
    }
    Ok(offset as usize..end as usize)
}
fn u16_at(b: &[u8], p: usize) -> u16 {
    u16::from_le_bytes(b[p..p + 2].try_into().unwrap())
}
fn u32_at(b: &[u8], p: usize) -> u32 {
    u32::from_le_bytes(b[p..p + 4].try_into().unwrap())
}
fn u64_at(b: &[u8], p: usize) -> u64 {
    u64::from_le_bytes(b[p..p + 8].try_into().unwrap())
}
pub fn read(path: &Path) -> Result<Layout> {
    let file = File::open(path)?;
    if file.metadata()?.len() > LIMIT {
        return Err(bad("elf-size"));
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1).read_to_end(&mut bytes)?;
    validate(&bytes)
}
pub fn validate(b: &[u8]) -> Result<Layout> {
    if b.len() > LIMIT as usize {
        return Err(bad("elf-size"));
    }
    if b.len() < 64 {
        return Err(bad("elf-header"));
    }
    if &b[..7] != b"\x7fELF\x02\x01\x01"
        || u16_at(b, 16) != 2
        || u16_at(b, 18) != 62
        || u32_at(b, 20) != 1
        || u16_at(b, 52) != 64
    {
        return Err(bad("elf-header"));
    }
    let entry = u64_at(b, 24);
    let offset = u64_at(b, 32);
    let count = u16_at(b, 56) as u64;
    if u16_at(b, 54) != 56 || !(1..=128).contains(&count) || offset < 64 {
        return Err(bad("elf-table"));
    }
    range(offset, count * 56, b.len())?;
    let mut previous_end = None;
    let mut file_ranges = Vec::new();
    let mut executable_entry = false;
    let mut loads = 0;
    for i in 0..count {
        let p = (offset + i * 56) as usize;
        let kind = u32_at(b, p);
        if u32_at(b, p + 4) & 3 == 3 {
            return Err(bad("elf-permissions"));
        }
        if kind == 2 || kind == 3 {
            return Err(bad("elf-dynamic"));
        }
        if kind != 1 {
            continue;
        }
        let flags = u32_at(b, p + 4);
        let file_offset = u64_at(b, p + 8);
        let address = u64_at(b, p + 16);
        let file_size = u64_at(b, p + 32);
        let memory_size = u64_at(b, p + 40);
        let align = u64_at(b, p + 48);
        if ![4, 5, 6].contains(&flags) {
            return Err(bad("elf-permissions"));
        }
        if file_size > memory_size || memory_size == 0 {
            return Err(bad("elf-size"));
        }
        range(file_offset, file_size, b.len())?;
        let end = address
            .checked_add(memory_size)
            .ok_or_else(|| bad("elf-overflow"))?;
        if align < 4096
            || !align.is_power_of_two()
            || !address.is_multiple_of(4096)
            || !file_offset.is_multiple_of(4096)
            || address % align != file_offset % align
        {
            return Err(bad("elf-alignment"));
        }
        if let Some(last) = previous_end {
            if address < last {
                return Err(bad("elf-overlap"));
            }
        } else if address != 0xffffffff80000000 || flags != 5 {
            return Err(bad("elf-base"));
        }
        if file_size != 0 {
            let file_end = file_offset + file_size;
            if file_ranges
                .iter()
                .any(|&(start, end)| file_offset < end && start < file_end)
            {
                return Err(bad("elf-overlap"));
            }
            file_ranges.push((file_offset, file_end));
        }
        previous_end = Some(end);
        executable_entry |= flags == 5 && entry >= address && entry < address + file_size;
        loads += 1;
    }
    if !executable_entry {
        return Err(bad("elf-entry"));
    }
    Ok(Layout { entry, loads })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put16(b: &mut [u8], p: usize, n: u16) {
        b[p..p + 2].copy_from_slice(&n.to_le_bytes());
    }
    fn put32(b: &mut [u8], p: usize, n: u32) {
        b[p..p + 4].copy_from_slice(&n.to_le_bytes());
    }
    fn put64(b: &mut [u8], p: usize, n: u64) {
        b[p..p + 8].copy_from_slice(&n.to_le_bytes());
    }
    // Hand-authored executable: one instruction byte at file 4096, linked at the specified base.
    fn fixture() -> Vec<u8> {
        let mut b = vec![0; 4097];
        b[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        put16(&mut b, 16, 2);
        put16(&mut b, 18, 62);
        put32(&mut b, 20, 1);
        put64(&mut b, 24, 0xffffffff80000000);
        put64(&mut b, 32, 64);
        put16(&mut b, 52, 64);
        put16(&mut b, 54, 56);
        put16(&mut b, 56, 1);
        put32(&mut b, 64, 1);
        put32(&mut b, 68, 5);
        put64(&mut b, 72, 4096);
        put64(&mut b, 80, 0xffffffff80000000);
        put64(&mut b, 96, 1);
        put64(&mut b, 104, 1);
        put64(&mut b, 112, 4096);
        b[4096] = 0xcc;
        b
    }
    #[test]
    fn accepts_static_kernel_without_empty_segments() {
        let layout = validate(&fixture()).unwrap();
        assert_eq!(layout.entry, 0xffffffff80000000);
        assert_eq!(layout.loads, 1);
    }
    #[test]
    fn accepts_separate_readonly_and_zero_fill_data() {
        let mut b = fixture();
        b.resize(12288, 0);
        put16(&mut b, 56, 3);
        b.copy_within(64..120, 120);
        b.copy_within(64..120, 176);
        put32(&mut b, 124, 4);
        put64(&mut b, 128, 8192);
        put64(&mut b, 136, 0xffffffff80001000);
        put32(&mut b, 180, 6);
        put64(&mut b, 184, 12288);
        put64(&mut b, 192, 0xffffffff80002000);
        put64(&mut b, 208, 0);
        put64(&mut b, 216, 4096);
        assert_eq!(validate(&b).unwrap().loads, 3);
    }
    #[test]
    fn rejects_malformed_headers_and_ranges() {
        assert_eq!(validate(&fixture()[..63]).unwrap_err().kind, "elf-header");
        let cases: Vec<(&str, usize, u64)> = vec![
            ("elf-bounds", 32, 4096),
            ("elf-overflow", 32, u64::MAX - 1),
            ("elf-size", 96, 2),
            ("elf-entry", 24, 0xffffffff80001000),
            ("elf-alignment", 112, 3),
            ("elf-alignment", 72, 1),
            ("elf-overflow", 80, u64::MAX),
            ("elf-bounds", 72, 8192),
        ];
        for (kind, p, value) in cases {
            let mut b = fixture();
            put64(&mut b, p, value);
            assert_eq!(validate(&b).unwrap_err().kind, kind, "offset {p}");
        }
        for (p, value, kind) in [
            (0, 0, "elf-header"),
            (18, 183, "elf-header"),
            (54, 64, "elf-table"),
            (56, 129, "elf-table"),
            (68, 7, "elf-permissions"),
            (64, 2, "elf-dynamic"),
            (64, 3, "elf-dynamic"),
        ] {
            let mut b = fixture();
            put16(&mut b, p, value);
            assert_eq!(validate(&b).unwrap_err().kind, kind, "offset {p}");
        }
    }
    #[test]
    fn rejects_overlapping_loads() {
        let mut b = fixture();
        put16(&mut b, 56, 2);
        b.copy_within(64..120, 120);
        assert_eq!(validate(&b).unwrap_err().kind, "elf-overlap");
        put64(&mut b, 136, 0xffffffff80001000);
        assert_eq!(validate(&b).unwrap_err().kind, "elf-overlap");
    }
}
