//! 对配置快照与 BAR 范围进行有界校验；不接触端口或设备。
use crate::boot_contract::Region;
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bar {
    pub base: u64,
    pub length: u64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Error {
    Identity,
    Bar,
    Range,
    Overlap,
    Capability,
}
impl Bar {
    pub fn contains(self, base: u64, size: usize) -> bool {
        size != 0
            && self.base <= base
            && base
                .checked_add(size as u64)
                .zip(self.base.checked_add(self.length))
                .is_some_and(|(e, l)| e <= l)
    }
}
pub fn bars(bars: &[Bar; 6], regions: &[Region]) -> Result<(), Error> {
    for (i, b) in bars.iter().enumerate().filter(|(_, b)| b.length != 0) {
        let end = b.base.checked_add(b.length).ok_or(Error::Range)?;
        if b.base < 512 * 1024 * 1024
            || !b.base.is_multiple_of(4096)
            || !b.length.is_power_of_two()
            || b.length < 4096
            || b.length > 16 * 1024 * 1024
            || !b.base.is_multiple_of(b.length)
            || end > 1 << 52
        {
            return Err(Error::Bar);
        }
        for r in regions {
            let re = r.base.checked_add(r.length).ok_or(Error::Range)?;
            if b.base < re && r.base < end {
                return Err(Error::Overlap);
            }
        }
        for other in &bars[..i] {
            if other.length != 0 && b.base < other.base + other.length && other.base < end {
                return Err(Error::Overlap);
            }
        }
    }
    Ok(())
}
pub fn capabilities(config: &[u8; 256], bars: &[Bar; 6]) -> Result<usize, Error> {
    if u16::from_le_bytes([config[0], config[1]]) != 0x1af4
        || u16::from_le_bytes([config[2], config[3]]) != 0x1042
        || config[11] != 1
        || config[10] != 0
        || config[14] & 0x7f != 0
    {
        return Err(Error::Identity);
    }
    if config[6] & 0x10 == 0 {
        return Err(Error::Capability);
    }
    let mut position = config[0x34] as usize;
    let mut seen = [false; 256];
    let mut required = 0u8;
    let mut regions = [(0u64, 0u64); 4];
    let mut count = 0;
    while position != 0 {
        if !(0x40..=0xfc).contains(&position) || !position.is_multiple_of(4) || seen[position] {
            return Err(Error::Capability);
        }
        seen[position] = true;
        count += 1;
        if config[position] == 9 {
            let length = config[position + 2] as usize;
            let kind = config[position + 3];
            if length < 16 || position + length > 256 {
                return Err(Error::Capability);
            }
            if (1..=4).contains(&kind) {
                let bar = config[position + 4] as usize;
                if bar >= 6 || bars[bar].length == 0 || required & (1 << kind) != 0 {
                    return Err(Error::Capability);
                }
                let offset =
                    u32::from_le_bytes(config[position + 8..position + 12].try_into().unwrap())
                        as u64;
                let size =
                    u32::from_le_bytes(config[position + 12..position + 16].try_into().unwrap())
                        as u64;
                let min = match kind {
                    1 => 56,
                    2 => 2,
                    3 => 1,
                    _ => 8,
                };
                if size < min
                    || offset
                        .checked_add(size)
                        .is_none_or(|e| e > bars[bar].length)
                    || (kind == 2 && length < 20)
                {
                    return Err(Error::Range);
                }
                let start = bars[bar].base + offset;
                let end = start + size;
                for (a, b) in regions {
                    if a < b && start < b && a < end {
                        return Err(Error::Overlap);
                    }
                }
                regions[(kind - 1) as usize] = (start, end);
                required |= 1 << kind;
            }
        }
        position = config[position + 1] as usize;
    }
    if required != 0b11110 {
        return Err(Error::Capability);
    }
    Ok(count)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bar_ranges_reject_ram_wrap_alignment_and_overlap() {
        let mut b = [Bar::default(); 6];
        b[0] = Bar {
            base: 0x80000000,
            length: 0x4000,
        };
        assert_eq!(bars(&b, &[]), Ok(()));
        assert_eq!(
            bars(
                &b,
                &[Region {
                    base: 0x80000000,
                    length: 0x1000,
                    kind: 7
                }]
            ),
            Err(Error::Overlap)
        );
        b[1] = b[0];
        assert_eq!(bars(&b, &[]), Err(Error::Overlap));
        b[1] = Bar::default();
        b[0].base = u64::MAX;
        assert_eq!(bars(&b, &[]), Err(Error::Range));
    }
    fn config() -> ([u8; 256], [Bar; 6]) {
        let mut c = [0; 256];
        c[0..4].copy_from_slice(&[0xf4, 0x1a, 0x42, 0x10]);
        c[11] = 1;
        c[6] = 0x10;
        c[0x34] = 0x40;
        for i in 0..4 {
            let p = 0x40 + i * 0x20;
            c[p] = 9;
            c[p + 1] = if i < 3 { (p + 0x20) as u8 } else { 0 };
            c[p + 2] = 20;
            c[p + 3] = (i + 1) as u8;
            c[p + 8..p + 12].copy_from_slice(&(i as u32 * 4096).to_le_bytes());
            c[p + 12..p + 16].copy_from_slice(&64u32.to_le_bytes());
        }
        let mut b = [Bar::default(); 6];
        b[0] = Bar {
            base: 0x90000000,
            length: 0x4000,
        };
        (c, b)
    }
    #[test]
    fn capabilities_are_bounded_and_nonoverlapping() {
        let (mut c, b) = config();
        assert_eq!(capabilities(&c, &b), Ok(4));
        c[0xa1] = 0x40;
        assert_eq!(capabilities(&c, &b), Err(Error::Capability));
        let (mut c, b) = config();
        c[0x48..0x4c].copy_from_slice(&0xfffffff0u32.to_le_bytes());
        assert_eq!(capabilities(&c, &b), Err(Error::Range));
        let (mut c, b) = config();
        c[0x68..0x6c].fill(0);
        assert_eq!(capabilities(&c, &b), Err(Error::Overlap));
    }
}
