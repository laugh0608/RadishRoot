use super::*;
use alloc::{string::String, vec};
fn key(code: KeyCode) -> Event {
    Event::Key {
        code,
        phase: Phase::Press,
        modifiers: Modifiers::default(),
    }
}
#[test]
fn input_limit_preserves_existing_utf8_and_backspace_removes_whole_character() {
    let mut v = View::new(Geometry::new(320, 240).unwrap());
    for _ in 0..126 {
        v.apply(Event::Text('a'));
    }
    v.apply(Event::Text('é'));
    assert_eq!(v.text().len(), 128);
    let before = String::from(v.text());
    v.apply(Event::Text('x'));
    assert_eq!(v.text(), before);
    assert_eq!(v.status, Status::TextFull);
    v.apply(key(KeyCode::Backspace));
    assert_eq!(v.text().len(), 126);
    v.apply(Event::Text('中'));
    assert_eq!(v.text().len(), 126);
    v.apply(key(KeyCode::Escape));
    assert!(v.text().is_empty());
}
#[test]
fn focus_activation_pointer_clamping_and_reset_are_explicit() {
    let mut v = View::new(Geometry::new(800, 600).unwrap());
    v.apply(key(KeyCode::Tab));
    v.apply(key(KeyCode::Return));
    assert_eq!(v.activations, [0, 1]);
    v.apply(Event::Text('x'));
    assert!(v.text().is_empty());
    v.apply(Event::PointerMove {
        dx: i16::MIN,
        dy: i16::MAX,
    });
    assert_eq!(v.pointer, Point::new(0, 599));
    v.apply(Event::PointerMove {
        dx: i16::MAX,
        dy: i16::MIN,
    });
    assert_eq!(v.pointer, Point::new(799, 0));
    v.pointer = Point::new(30, 160);
    v.apply(Event::PointerButton { down: true });
    assert_eq!(v.focus, 0);
    v.apply(Event::InputReset);
    assert!(!v.left);
    assert_eq!(v.resets, 1);
    v.apply(Event::Text('a'));
    assert_eq!(v.text(), "a");
}
#[test]
fn actual_pixels_have_independent_corners_glyph_and_restore_old_cursor() {
    let g = Geometry::new(320, 240).unwrap();
    let mut pixels = vec![0; g.pixels()];
    let mut s = Surface::new(g, &mut pixels).unwrap();
    let mut v = View::new(g);
    v.render(&mut s);
    assert!(!v.dirty());
    assert_eq!(s.pixels()[0], 0xff0000);
    assert_eq!(s.pixels()[319], 0x00ff00);
    assert_eq!(s.pixels()[239 * 320], 0x0000ff);
    assert_eq!(s.pixels()[240 * 320 - 1], 0xffffff);
    // 独立 BDF 的 R 为 00/F0/88/...；baseline=7，所以 baseline 21 的 F0 行在 y=15。
    let row = &s.pixels()[15 * 320 + 20..15 * 320 + 26];
    assert_eq!(
        row,
        [0x202f2a, 0x202f2a, 0x202f2a, 0x202f2a, 0xfcfdfb, 0xfcfdfb]
    );
    v.apply(Event::PointerMove { dx: 10, dy: 0 });
    assert!(v.dirty());
    v.render(&mut s);
    assert_eq!(s.pixels()[120 * 320 + 160], 0xfcfdfb);
}
