use super::*;
use alloc::vec;

fn description() -> Framebuffer {
    Framebuffer {
        physical: 0xe0000000,
        width: 320,
        height: 240,
        pitch: 1296,
        bpp: 32,
        model: 1,
        masks: [(8, 16), (8, 8), (8, 0)],
    }
}
#[test]
fn padded_rows_and_color_order_are_independent_of_backing_format() {
    let mut fb = description();
    for (shifts, expected) in [
        ([(8, 16), (8, 8), (8, 0)], [0x56, 0x34, 0x12, 0]),
        ([(8, 0), (8, 8), (8, 16)], [0x12, 0x34, 0x56, 0]),
    ] {
        fb.masks = shifts;
        let format = Format::new(fb).unwrap();
        let mut output = vec![0xcd; format.length + 16];
        format
            .present(&vec![0x123456; 320 * 240], &mut output)
            .unwrap();
        for row in output[..format.length].chunks_exact(1296) {
            assert_eq!(&row[..4], &expected);
            assert_eq!(&row[1276..1280], &expected);
            assert_eq!(&row[1280..], &[0xcd; 16]);
        }
        assert_eq!(&output[format.length..], &[0xcd; 16]);
    }
}
#[test]
fn clips_signed_and_extreme_rectangles_before_indexing() {
    let geometry = Geometry::new(320, 240).unwrap();
    let mut data = vec![0xdeadbeef; geometry.pixels() + 2];
    {
        let mut s = Surface::new(geometry, &mut data[1..=geometry.pixels()]).unwrap();
        s.clear(Rgb888::BLACK).unwrap();
        s.rectangle(Point::new(-2, -2), Size::new(4, 4), Rgb888::RED);
        s.rectangle(
            Point::new(319, 239),
            Size::new(u32::MAX, u32::MAX),
            Rgb888::BLUE,
        );
        s.pixel(Point::new(i32::MIN, i32::MAX), Rgb888::GREEN);
        assert_eq!(&s.pixels()[..4], &[0xff0000, 0xff0000, 0, 0]);
        assert_eq!(s.pixels()[320], 0xff0000);
        assert_eq!(s.pixels()[640], 0);
        assert_eq!(s.pixels()[320 * 240 - 1], 0xff);
        s.rectangle(
            Point::new(i32::MIN, i32::MIN),
            Size::new(u32::MAX, u32::MAX),
            Rgb888::GREEN,
        );
        assert!(s.pixels().iter().all(|c| *c == 0x00ff00));
    }
    assert_eq!(data[0], 0xdeadbeef);
    assert_eq!(data[geometry.pixels() + 1], 0xdeadbeef);
}
#[test]
fn rejects_unsupported_ranges_without_writes() {
    assert_eq!(Geometry::new(319, 240), Err(Error::Dimensions));
    assert_eq!(Geometry::new(320, 239), Err(Error::Dimensions));
    assert!(Geometry::new(u64::MAX, u64::MAX).is_err());
    assert_eq!(Geometry::new(4096, 4096), Err(Error::Capacity));
    assert_eq!(
        Geometry::new(1024, 768).unwrap().allocation_bytes(),
        770 * 4096
    );
    for change in 0..4 {
        let mut fb = description();
        match change {
            0 => fb.pitch = 1279,
            1 => fb.masks[0] = (8, 8),
            2 => fb.physical = u64::MAX,
            _ => fb.bpp = 24,
        }
        assert!(Format::new(fb).is_err());
    }
    let f = Format::new(description()).unwrap();
    let mut bytes = vec![0xcc; f.length];
    assert_eq!(f.present(&[0; 1], &mut bytes), Err(Error::Capacity));
    assert!(bytes.iter().all(|b| *b == 0xcc));
}
