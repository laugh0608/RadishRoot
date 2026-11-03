//! 工作台输入投影的有界回放；其它普通键的 Key 事件等价为空动作，Text 单独保留。
use crate::input::{Event, KeyCode, Modifiers, Phase};
use core::fmt::{self, Write};
const KEYS: &[(&str, KeyCode)] = &[
    ("tab", KeyCode::Tab),
    ("enter", KeyCode::Return),
    ("numpad-enter", KeyCode::NumpadEnter),
    ("escape", KeyCode::Escape),
    ("backspace", KeyCode::Backspace),
    ("delete", KeyCode::Delete),
    ("left", KeyCode::ArrowLeft),
    ("right", KeyCode::ArrowRight),
    ("up", KeyCode::ArrowUp),
    ("down", KeyCode::ArrowDown),
    ("home", KeyCode::Home),
    ("end", KeyCode::End),
    ("c", KeyCode::C),
    ("other", KeyCode::A),
];
pub fn write_event(w: &mut impl Write, e: Event) -> fmt::Result {
    match e {
        Event::Text(c) => write!(w, "text {}", c as u32),
        Event::Key {
            code,
            phase,
            modifiers: m,
        } => write!(
            w,
            "key {} {} {}",
            KEYS.iter()
                .find(|(_, c)| *c == code)
                .map_or("other", |(s, _)| *s),
            match phase {
                Phase::Press => 0,
                Phase::Repeat => 1,
                Phase::Release => 2,
            },
            u8::from(m.shift)
                | u8::from(m.control) << 1
                | u8::from(m.alt) << 2
                | u8::from(m.caps) << 3
        ),
        Event::PointerMove { dx, dy } => write!(w, "move {dx} {dy}"),
        Event::PointerButton { down } => write!(w, "button {}", u8::from(down)),
        Event::InputReset => write!(w, "reset"),
    }
}
pub fn parse(s: &str) -> Option<Event> {
    let mut p = s.split_whitespace();
    let event = match p.next()? {
        "text" => Event::Text(char::from_u32(p.next()?.parse().ok()?)?),
        "key" => {
            let name = p.next()?;
            let code = KEYS.iter().find(|(s, _)| *s == name)?.1;
            let phase = match p.next()? {
                "0" => Phase::Press,
                "1" => Phase::Repeat,
                "2" => Phase::Release,
                _ => return None,
            };
            let m = p.next()?.parse::<u8>().ok()?;
            if m > 15 {
                return None;
            }
            Event::Key {
                code,
                phase,
                modifiers: Modifiers {
                    shift: m & 1 != 0,
                    control: m & 2 != 0,
                    alt: m & 4 != 0,
                    caps: m & 8 != 0,
                },
            }
        }
        "move" => Event::PointerMove {
            dx: p.next()?.parse().ok()?,
            dy: p.next()?.parse().ok()?,
        },
        "button" => Event::PointerButton {
            down: match p.next()? {
                "0" => false,
                "1" => true,
                _ => return None,
            },
        },
        "reset" => Event::InputReset,
        _ => return None,
    };
    p.next().is_none().then_some(event)
}
