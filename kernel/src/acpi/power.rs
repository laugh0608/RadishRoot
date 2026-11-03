//! 锁定 QEMU q35 的 FADT 布局；S5=0，reset 值由表提供，不扫描 AML 字节串。
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Registers {
    pub pm1_control: u16,
    pub reset_port: u16,
    pub reset_value: u8,
}
pub fn q35(b: &[u8]) -> Result<Registers, Error> {
    table(b, b"FACP")?;
    if b.len() < 129 {
        return Err(Error::Length);
    }
    let word = |at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    let reset = &b[116..128];
    if word(68) != 0
        || word(64) != 0x604
        || b[89] != 2
        || word(112) & (1 << 10) == 0
        || reset[..3] != [1, 8, 0]
        || ![0, 1].contains(&reset[3])
        || u64::from_le_bytes(reset[4..12].try_into().unwrap()) != 0xcf9
        || b[128] != 0xf
    {
        return Err(Error::Unsupported);
    }
    Ok(Registers {
        pm1_control: 0x604,
        reset_port: 0xcf9,
        reset_value: b[128],
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fix(b: &mut [u8]) {
        b[9] = 0;
        b[9] = 0u8.wrapping_sub(b.iter().fold(0u8, |a, b| a.wrapping_add(*b)));
    }
    #[test]
    fn accepts_only_checksummed_fixed_register_geometry() {
        let mut b = [0u8; 276];
        b[..4].copy_from_slice(b"FACP");
        b[4..8].copy_from_slice(&276u32.to_le_bytes());
        b[64..68].copy_from_slice(&0x604u32.to_le_bytes());
        b[89] = 2;
        b[113] = 4;
        b[116..119].copy_from_slice(&[1, 8, 0]);
        b[120..128].copy_from_slice(&0xcf9u64.to_le_bytes());
        b[128] = 0xf;
        fix(&mut b);
        assert_eq!(q35(&b).unwrap().pm1_control, 0x604);
        for (offset, value) in [
            (64, 5),
            (68, 1),
            (89, 4),
            (113, 0),
            (116, 0),
            (117, 16),
            (118, 1),
            (119, 4),
            (128, 6),
        ] {
            let mut changed = b;
            changed[offset] = value;
            fix(&mut changed);
            assert_eq!(q35(&changed), Err(Error::Unsupported));
        }
        b[9] ^= 1;
        assert_eq!(q35(&b), Err(Error::Checksum));
        assert_eq!(q35(&b[..128]), Err(Error::Length));
    }
}
