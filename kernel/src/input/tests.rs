use super::*;
use alloc::vec::Vec;
#[test]
fn raw_overflow_latches_outside_full_queue_and_discards_the_lost_interval() {
    let mut queue = RawQueue::new();
    let shift = Raw {
        port: Port::Keyboard,
        byte: 0x12,
        errors: 0,
    };
    for _ in 0..RAW_CAPACITY {
        queue.push(shift);
    }
    queue.push(Raw {
        byte: 0xf0,
        ..shift
    });
    let batch = queue.drain(false);
    assert!(batch.lost);
    assert_eq!(batch.dropped, 1);
    assert_eq!(batch.received, 257);
    assert!(batch.items.iter().all(Option::is_none));
    queue.clear_after_reset();
    queue.push(Raw {
        byte: 0x1c,
        ..shift
    });
    let batch = queue.drain(false);
    assert!(!batch.lost);
    assert_eq!(batch.dropped, 1);
    let mut decoder = Decoder::new();
    assert_eq!(
        decoder.feed(batch.items[0].unwrap()).unwrap()[1],
        Some(Event::Text('a'))
    );
    assert!(batch.items[1..].iter().all(Option::is_none));
}
#[test]
fn replies_are_port_and_phase_specific_and_never_ordinary_text() {
    let ack = Raw {
        port: Port::Mouse,
        byte: 0xfa,
        errors: 0,
    };
    assert_eq!(transaction_byte(ack, Port::Keyboard), Err("PS2_REPLY_PORT"));
    assert_eq!(transaction_byte(ack, Port::Mouse), Ok(0xfa));
    assert_eq!(
        transaction_byte(
            Raw {
                errors: 0x40,
                ..ack
            },
            Port::Mouse
        ),
        Err("PS2_STATUS_ERROR")
    );
    let mut decoder = Decoder::new();
    assert_eq!(
        decoder.feed(Raw {
            port: Port::Keyboard,
            ..ack
        }),
        Err(Error::ReplyInStream)
    );
}
fn feed(d: &mut Decoder, port: Port, bytes: &[u8]) -> Vec<Event> {
    bytes
        .iter()
        .flat_map(|b| {
            d.feed(Raw {
                port,
                byte: *b,
                errors: 0,
            })
            .unwrap()
            .into_iter()
            .flatten()
        })
        .collect()
}
#[test]
fn release_repeat_modifiers_and_controls_have_distinct_semantics() {
    let mut d = Decoder::new();
    let events = feed(
        &mut d,
        Port::Keyboard,
        &[
            0x12, 0x1c, 0x1c, 0xf0, 0x1c, 0xf0, 0x12, 0x58, 0x58, 0xf0, 0x58, 0x1c, 0xf0, 0x1c,
        ],
    );
    let text = events
        .iter()
        .filter_map(|e| {
            if let Event::Text(c) = e {
                Some(*c)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(text, ['A', 'A', 'A']);
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Key {
            code: KeyCode::A,
            phase: Phase::Repeat,
            ..
        }
    )));
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(
                e,
                Event::Key {
                    code: KeyCode::A,
                    phase: Phase::Release,
                    ..
                }
            ))
            .count(),
        2
    );
    assert_eq!(
        d.modifiers(),
        Modifiers {
            caps: true,
            ..Modifiers::default()
        }
    );
    for modifier in [0x14, 0x11] {
        let mut d = Decoder::new();
        let ev = feed(
            &mut d,
            Port::Keyboard,
            &[modifier, 0x1c, 0xf0, 0x1c, 0xf0, modifier],
        );
        assert!(!ev.iter().any(|e| matches!(e, Event::Text(_))));
        assert_eq!(d.modifiers(), Modifiers::default());
    }
    let ev = feed(
        &mut d,
        Port::Keyboard,
        &[
            0xe0, 0x75, 0xe0, 0xf0, 0x75, 0x66, 0xf0, 0x66, 0x0d, 0x5a, 0x76,
        ],
    );
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::Key {
            code: KeyCode::ArrowUp,
            phase: Phase::Release,
            ..
        }
    )));
    assert!(!ev.iter().any(|e| matches!(e, Event::Text(_))));
}
#[test]
fn mouse_preserves_negative_displacement_y_and_button_edges() {
    let mut d = Decoder::new();
    let ev = feed(&mut d, Port::Mouse, &[0x39, 0xfe, 0xfd, 0x08, 1, 2]);
    assert_eq!(
        ev,
        [
            Event::PointerMove { dx: -2, dy: 3 },
            Event::PointerButton { down: true },
            Event::PointerMove { dx: 1, dy: -2 },
            Event::PointerButton { down: false }
        ]
    );
    assert!(feed(&mut d, Port::Mouse, &[0x08, 0, 0]).is_empty());
    // 九位位移的符号来自 flags，不可把 data 直接解释成 i8。
    assert_eq!(
        feed(&mut d, Port::Mouse, &[0x08, 200, 0]),
        [Event::PointerMove { dx: 200, dy: 0 }]
    );
}
#[test]
fn corruption_requires_reset_and_new_stream_does_not_retain_modifiers() {
    let mut d = Decoder::new();
    feed(&mut d, Port::Keyboard, &[0x12]);
    assert_eq!(
        d.feed(Raw {
            port: Port::Keyboard,
            byte: 0xfa,
            errors: 0
        }),
        Err(Error::ReplyInStream)
    );
    d = Decoder::new();
    assert_eq!(
        feed(&mut d, Port::Keyboard, &[0x1c]).last(),
        Some(&Event::Text('a'))
    );
    assert_eq!(
        d.feed(Raw {
            port: Port::Mouse,
            byte: 0,
            errors: 0
        }),
        Err(Error::MouseSync)
    );
    feed(&mut d, Port::Mouse, &[0x48, 0]);
    assert_eq!(
        d.feed(Raw {
            port: Port::Mouse,
            byte: 0,
            errors: 0
        }),
        Err(Error::MouseOverflow)
    );
    assert_eq!(
        d.feed(Raw {
            port: Port::Keyboard,
            byte: 0x1c,
            errors: 0x80
        }),
        Err(Error::Controller)
    );
}
