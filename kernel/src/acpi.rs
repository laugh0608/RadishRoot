//! 仅解析 HPET 所需的 ACPI 2+ 表；不访问硬件、不执行 AML。
pub const MAX_TABLE: usize = 65536;
pub const MAX_ROOT_ENTRIES: usize = 256;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Length,
    Signature,
    Checksum,
    Address,
    Unsupported,
}
fn checksum(b: &[u8]) -> bool {
    b.iter().fold(0u8, |s, n| s.wrapping_add(*n)) == 0
}
pub fn rsdp(b: &[u8]) -> Result<u64, Error> {
    if b.len() < 36 {
        return Err(Error::Length);
    }
    if &b[..8] != b"RSD PTR " {
        return Err(Error::Signature);
    }
    if b[15] < 2 {
        return Err(Error::Unsupported);
    }
    let length = u32::from_le_bytes(b[20..24].try_into().unwrap()) as usize;
    if !(36..=MAX_TABLE).contains(&length) || b.len() != length {
        return Err(Error::Length);
    }
    if !checksum(&b[..20]) || !checksum(b) {
        return Err(Error::Checksum);
    }
    let address = u64::from_le_bytes(b[24..32].try_into().unwrap());
    if address == 0 {
        return Err(Error::Address);
    }
    Ok(address)
}
pub fn table_length(header: &[u8]) -> Result<usize, Error> {
    if header.len() < 36 {
        return Err(Error::Length);
    }
    let length = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
    if !(36..=MAX_TABLE).contains(&length) {
        return Err(Error::Length);
    }
    Ok(length)
}
pub fn table<'a>(b: &'a [u8], signature: &[u8; 4]) -> Result<&'a [u8], Error> {
    if table_length(b)? != b.len() {
        return Err(Error::Length);
    }
    if &b[..4] != signature {
        return Err(Error::Signature);
    }
    if !checksum(b) {
        return Err(Error::Checksum);
    }
    Ok(&b[36..])
}
pub fn xsdt(b: &[u8]) -> Result<&[u8], Error> {
    let entries = table(b, b"XSDT")?;
    if !entries.len().is_multiple_of(8) || entries.len() / 8 > MAX_ROOT_ENTRIES {
        return Err(Error::Length);
    }
    Ok(entries)
}
pub fn hpet(b: &[u8]) -> Result<u64, Error> {
    let data = table(b, b"HPET")?;
    if data.len() != 20 {
        return Err(Error::Length);
    }
    // GAS 位于表偏移40；System Memory、零 bit offset；宽度按 HPET 描述允许 0/64。
    if data[4] != 0 || ![0, 64].contains(&data[5]) || data[6] != 0 || ![0, 4].contains(&data[7]) {
        return Err(Error::Unsupported);
    }
    let address = u64::from_le_bytes(data[8..16].try_into().unwrap());
    if address == 0
        || !address.is_multiple_of(4096)
        || address.checked_add(4096).is_none_or(|e| e > (1 << 52))
    {
        return Err(Error::Address);
    }
    Ok(address)
}
pub fn mapped_table_range(
    base: u64,
    length: usize,
    regions: &[crate::boot_contract::Region],
) -> bool {
    length != 0
        && length <= MAX_TABLE
        && base.checked_add(length as u64).is_some_and(|end| {
            regions.iter().any(|r| {
                matches!(r.kind, 2 | 3 | 8)
                    && r.base <= base
                    && r.base.checked_add(r.length).is_some_and(|e| end <= e)
            })
        })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fix(b: &mut [u8], offset: usize) {
        b[offset] = 0;
        b[offset] = 0u8.wrapping_sub(b.iter().fold(0u8, |s, n| s.wrapping_add(*n)));
    }
    #[test]
    fn rejects_truncation_bad_checksum_geometry_and_gas() {
        let mut b = [0u8; 56];
        b[..4].copy_from_slice(b"HPET");
        b[4..8].copy_from_slice(&56u32.to_le_bytes());
        b[44..52].copy_from_slice(&0xfed00000u64.to_le_bytes());
        fix(&mut b, 9);
        assert_eq!(hpet(&b), Ok(0xfed00000));
        assert_eq!(hpet(&b[..55]), Err(Error::Length));
        b[55] ^= 1;
        assert_eq!(hpet(&b), Err(Error::Checksum));
        b[55] ^= 1;
        b[40] = 1;
        fix(&mut b, 9);
        assert_eq!(hpet(&b), Err(Error::Unsupported));
        b[40] = 0;
        b[44] = 1;
        fix(&mut b, 9);
        assert_eq!(hpet(&b), Err(Error::Address));
        b[4..8].copy_from_slice(&65537u32.to_le_bytes());
        assert_eq!(table_length(&b), Err(Error::Length));
    }
    #[test]
    fn rsdp_requires_both_checksums_and_nonzero_xsdt() {
        let mut b = [0u8; 36];
        b[..8].copy_from_slice(b"RSD PTR ");
        b[15] = 2;
        b[20..24].copy_from_slice(&36u32.to_le_bytes());
        b[24..32].copy_from_slice(&0x12340u64.to_le_bytes());
        fix(&mut b[..20], 8);
        fix(&mut b, 32);
        assert_eq!(rsdp(&b), Ok(0x12340));
        b[8] ^= 1;
        assert_eq!(rsdp(&b), Err(Error::Checksum));
        assert_eq!(rsdp(&b[..20]), Err(Error::Length));
    }
    #[test]
    fn ranges_cannot_cross_regions_overflow_or_use_ram() {
        use crate::boot_contract::Region;
        let r = [Region {
            base: 4096,
            length: 4096,
            kind: 2,
        }];
        assert!(mapped_table_range(4096, 4096, &r));
        assert!(!mapped_table_range(4097, 4096, &r));
        assert!(!mapped_table_range(u64::MAX, 36, &r));
        assert!(!mapped_table_range(4096, 36, &[Region { kind: 0, ..r[0] }]));
    }
    #[test]
    fn root_entry_count_and_alignment_are_bounded() {
        let mut b = [0u8; 36 + 257 * 8];
        b[..4].copy_from_slice(b"XSDT");
        let length = b.len() as u32;
        b[4..8].copy_from_slice(&length.to_le_bytes());
        fix(&mut b, 9);
        assert_eq!(xsdt(&b), Err(Error::Length));
        let length = 36 + 256 * 8;
        b[4..8].copy_from_slice(&(length as u32).to_le_bytes());
        fix(&mut b[..length], 9);
        assert_eq!(xsdt(&b[..length]).unwrap().len(), 256 * 8);
        let length = 37;
        b[4..8].copy_from_slice(&(length as u32).to_le_bytes());
        fix(&mut b[..length], 9);
        assert_eq!(xsdt(&b[..length]), Err(Error::Length));
    }
}
