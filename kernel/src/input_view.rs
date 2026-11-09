//! T07 输入 / 显示校验页：宿主与 guest 共用状态和绘制，没有文件服务或模拟文件列表。
use crate::{
    input::{Event, KeyCode, Modifiers, Phase, TEXT_CAPACITY},
    surface::{Geometry, Surface},
};
use core::fmt::{self, Write};
use embedded_graphics::{
    mono_font::{
        MonoTextStyle,
        ascii::{FONT_6X10, FONT_10X20},
    },
    pixelcolor::Rgb888,
    prelude::*,
    text::Text,
};

pub mod style {
    use embedded_graphics::pixelcolor::Rgb888;
    pub const BACKGROUND: Rgb888 = Rgb888::new(239, 242, 239);
    pub const SURFACE: Rgb888 = Rgb888::new(252, 253, 251);
    pub const TEXT: Rgb888 = Rgb888::new(32, 47, 42);
    pub const MUTED: Rgb888 = Rgb888::new(91, 108, 99);
    pub const BORDER: Rgb888 = Rgb888::new(176, 190, 181);
    pub const FOCUS: Rgb888 = Rgb888::new(36, 99, 80);
    pub const ACCENT: Rgb888 = Rgb888::new(64, 115, 93);
    pub const SUCCESS: Rgb888 = Rgb888::new(38, 111, 66);
    pub const WARNING: Rgb888 = Rgb888::new(134, 89, 27);
    pub const ERROR: Rgb888 = Rgb888::new(161, 44, 42);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ready,
    TextFull,
    InputReset,
    InputUnavailable,
}
impl Status {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ready => "Ready - input and display check",
            Self::TextFull => "Text limit: 128 bytes. Input rejected.",
            Self::InputReset => "Input lost: state reset. Release keys.",
            Self::InputUnavailable => "Input unavailable - see serial reason",
        }
    }
}
pub struct View {
    geometry: Geometry,
    text: [u8; TEXT_CAPACITY],
    length: usize,
    pub pointer: Point,
    pub left: bool,
    pub focus: u8,
    pub activations: [u64; 2],
    pub modifiers: Modifiers,
    pub status: Status,
    pub events: u64,
    pub releases: u64,
    pub resets: u64,
    pub button_presses: u64,
    pub button_releases: u64,
    pub pitch: usize,
    pub shifts: [u8; 3],
    dirty: bool,
}
impl View {
    pub fn new(geometry: Geometry) -> Self {
        Self {
            geometry,
            text: [0; TEXT_CAPACITY],
            length: 0,
            pointer: Point::new(geometry.width() as i32 / 2, geometry.height() as i32 / 2),
            left: false,
            focus: 0,
            activations: [0; 2],
            modifiers: Modifiers::default(),
            status: Status::Ready,
            events: 0,
            releases: 0,
            resets: 0,
            button_presses: 0,
            button_releases: 0,
            pitch: geometry.width() as usize * 4,
            shifts: [16, 8, 0],
            dirty: true,
        }
    }
    pub fn text(&self) -> &str {
        core::str::from_utf8(&self.text[..self.length]).expect("input stores complete UTF-8 chars")
    }
    pub const fn geometry(&self) -> Geometry {
        self.geometry
    }
    pub fn dirty(&self) -> bool {
        self.dirty
    }
    pub fn unavailable(&mut self) {
        self.status = Status::InputUnavailable;
        self.dirty = true;
    }
    pub fn input_rect(&self) -> (Point, Size) {
        let large = large(self.geometry);
        (
            Point::new(20, if large { 142 } else { 86 }),
            Size::new(
                self.geometry.width() - 40,
                if large {
                    160
                } else {
                    (self.geometry.height() - 180).min(120)
                },
            ),
        )
    }
    pub fn button_rect(&self) -> (Point, Size) {
        let (p, s) = self.input_rect();
        (
            Point::new(20, p.y + s.height as i32 + 14),
            Size::new(
                if large(self.geometry) { 240 } else { 180 },
                if large(self.geometry) { 40 } else { 28 },
            ),
        )
    }
    pub fn apply(&mut self, event: Event) {
        self.events = self.events.saturating_add(1);
        // 状态计数也是可见内容；每轮主循环合并为一帧，而非每个事件单独 present。
        self.dirty = true;
        match event {
            Event::Text(c) if self.focus == 0 && !c.is_control() => {
                let mut bytes = [0; 4];
                let value = c.encode_utf8(&mut bytes).as_bytes();
                if self.length + value.len() > TEXT_CAPACITY {
                    self.status = Status::TextFull;
                } else {
                    self.text[self.length..self.length + value.len()].copy_from_slice(value);
                    self.length += value.len();
                }
            }
            Event::Key {
                code,
                phase,
                modifiers,
            } => {
                self.modifiers = modifiers;
                if phase == Phase::Release {
                    self.releases = self.releases.saturating_add(1);
                    return;
                }
                if modifiers.control || modifiers.alt {
                    return;
                }
                match code {
                    KeyCode::Tab if phase == Phase::Press => self.focus ^= 1,
                    KeyCode::Return | KeyCode::NumpadEnter if phase == Phase::Press => {
                        self.activations[self.focus as usize] =
                            self.activations[self.focus as usize].saturating_add(1);
                    }
                    KeyCode::Escape => {
                        self.length = 0;
                        if self.status == Status::TextFull {
                            self.status = Status::Ready;
                        }
                    }
                    KeyCode::Backspace if self.focus == 0 => {
                        if let Some((index, _)) = self.text().char_indices().next_back() {
                            self.length = index;
                        }
                        if self.status == Status::TextFull {
                            self.status = Status::Ready;
                        }
                    }
                    _ => {}
                }
            }
            Event::PointerMove { dx, dy } => {
                self.pointer.x = (i64::from(self.pointer.x) + i64::from(dx))
                    .clamp(0, i64::from(self.geometry.width() - 1))
                    as i32;
                self.pointer.y = (i64::from(self.pointer.y) + i64::from(dy))
                    .clamp(0, i64::from(self.geometry.height() - 1))
                    as i32;
            }
            Event::PointerButton { down } => {
                if down && !self.left {
                    self.button_presses = self.button_presses.saturating_add(1);
                }
                if !down && self.left {
                    self.button_releases = self.button_releases.saturating_add(1);
                }
                if down && !self.left {
                    if contains(self.input_rect(), self.pointer) {
                        self.focus = 0;
                    }
                    if contains(self.button_rect(), self.pointer) {
                        self.focus = 1;
                    }
                }
                self.left = down;
            }
            Event::InputReset => {
                self.modifiers = Modifiers::default();
                self.left = false;
                self.resets = self.resets.saturating_add(1);
                self.status = Status::InputReset;
            }
            _ => {}
        }
    }
    pub fn render(&mut self, surface: &mut Surface<'_>) {
        assert_eq!(surface.geometry(), self.geometry);
        surface.clear(style::BACKGROUND).unwrap();
        let w = self.geometry.width();
        let h = self.geometry.height();
        let large = large(self.geometry);
        surface.rectangle(
            Point::zero(),
            Size::new(w, if large { 72 } else { 48 }),
            style::SURFACE,
        );
        label(
            surface,
            20,
            if large { 26 } else { 21 },
            "RADISHROOT / INPUT + DISPLAY",
            style::TEXT,
        );
        label(
            surface,
            20,
            if large { 52 } else { 37 },
            "T07 hardware foundation",
            style::MUTED,
        );
        let mut line = Line::new();
        write!(
            line,
            "{}x{} RGB32 pitch={} R/G/B={}/{}/{}",
            w, h, self.pitch, self.shifts[0], self.shifts[1], self.shifts[2]
        )
        .unwrap();
        label(
            surface,
            20,
            if large { 98 } else { 65 },
            line.as_str(),
            style::MUTED,
        );
        label(
            surface,
            20,
            if large { 124 } else { 79 },
            "Tab: focus / Enter: activate / Esc: clear",
            style::MUTED,
        );
        for (index, color) in [Rgb888::RED, Rgb888::GREEN, Rgb888::BLUE]
            .into_iter()
            .enumerate()
        {
            surface.rectangle(
                Point::new(w as i32 - 72 + index as i32 * 18, 16),
                Size::new(12, 12),
                color,
            );
        }
        for (index, rect) in [self.input_rect(), self.button_rect()]
            .into_iter()
            .enumerate()
        {
            let (p, size) = rect;
            surface.rectangle(
                p,
                size,
                if self.focus as usize == index {
                    style::FOCUS
                } else {
                    style::BORDER
                },
            );
            surface.rectangle(
                p + Point::new(2, 2),
                Size::new(size.width - 4, size.height - 4),
                style::SURFACE,
            );
        }
        let (p, size) = self.input_rect();
        let char_width = if large { 10 } else { 6 };
        let line_height = if large { 24 } else { 12 };
        let baseline = if large { 24 } else { 14 };
        let columns = ((size.width - 16) / char_width) as usize;
        let rows = ((size.height - baseline) / line_height) as usize;
        let skip = self.text().chars().count().saturating_sub(columns * rows);
        for (i, c) in self.text().chars().skip(skip).enumerate() {
            let mut bytes = [0; 4];
            let shown = if c.is_ascii() { c } else { '?' };
            label(
                surface,
                p.x + 8 + (i % columns) as i32 * char_width as i32,
                p.y + baseline as i32 + (i / columns) as i32 * line_height as i32,
                shown.encode_utf8(&mut bytes),
                style::TEXT,
            );
        }
        let (b, _) = self.button_rect();
        label(
            surface,
            b.x + 9,
            b.y + if large { 26 } else { 18 },
            if self.focus == 1 {
                "> Activate (Enter)"
            } else {
                "Activate (Enter)"
            },
            style::ACCENT,
        );
        let mut line = Line::new();
        write!(
            line,
            "focus={} bytes={} activated={}/{}",
            self.focus, self.length, self.activations[0], self.activations[1]
        )
        .unwrap();
        label(
            surface,
            20,
            b.y + if large { 68 } else { 44 },
            line.as_str(),
            style::MUTED,
        );
        let mut line = Line::new();
        write!(
            line,
            "pointer={},{} left={} shift={} caps={}",
            self.pointer.x,
            self.pointer.y,
            u8::from(self.left),
            u8::from(self.modifiers.shift),
            u8::from(self.modifiers.caps)
        )
        .unwrap();
        label(
            surface,
            20,
            h as i32 - if large { 54 } else { 38 },
            line.as_str(),
            style::MUTED,
        );
        label(
            surface,
            20,
            h as i32 - 22,
            self.status.label(),
            match self.status {
                Status::Ready => style::SUCCESS,
                Status::TextFull => style::WARNING,
                _ => style::ERROR,
            },
        );
        // 四角基准同时验证裁剪、色序和真实显示尺寸。
        for (point, color) in [
            (Point::new(0, 0), Rgb888::RED),
            (Point::new(w as i32 - 1, 0), Rgb888::GREEN),
            (Point::new(0, h as i32 - 1), Rgb888::BLUE),
            (Point::new(w as i32 - 1, h as i32 - 1), Rgb888::WHITE),
        ] {
            surface.pixel(point, color);
        }
        // 轮廓与填充均经后台；先完整重画，光标移动不会留下旧像素。
        for y in 0..12 {
            for x in 0..=y / 2 {
                surface.pixel(
                    self.pointer + Point::new(x, y),
                    if x == 0 || x == y / 2 || y == 11 {
                        style::TEXT
                    } else {
                        style::SURFACE
                    },
                );
            }
        }
        self.dirty = false;
    }
}
fn contains((p, s): (Point, Size), q: Point) -> bool {
    q.x >= p.x
        && q.y >= p.y
        && i64::from(q.x) < i64::from(p.x) + i64::from(s.width)
        && i64::from(q.y) < i64::from(p.y) + i64::from(s.height)
}
fn label(s: &mut Surface<'_>, x: i32, y: i32, text: &str, color: Rgb888) {
    let font = if large(s.geometry()) {
        &FONT_10X20
    } else {
        &FONT_6X10
    };
    Text::new(text, Point::new(x, y), MonoTextStyle::new(font, color))
        .draw(s)
        .unwrap();
}
fn large(geometry: Geometry) -> bool {
    geometry.width() >= 800 && geometry.height() >= 600
}
struct Line {
    bytes: [u8; 256],
    length: usize,
}
impl Line {
    fn new() -> Self {
        Self {
            bytes: [0; 256],
            length: 0,
        }
    }
    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.length]).expect("formatted UTF-8")
    }
}
impl fmt::Write for Line {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if self.length + s.len() > self.bytes.len() {
            return Err(fmt::Error);
        }
        self.bytes[self.length..self.length + s.len()].copy_from_slice(s.as_bytes());
        self.length += s.len();
        Ok(())
    }
}

#[cfg(test)]
mod tests;
