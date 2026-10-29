//! i8042 字节到类型化事件；ISR 不使用解码器，丢失后的状态由主循环显式重建。
pub use pc_keyboard::KeyCode;
use pc_keyboard::{
    DecodedKey, HandleControl, KeyState, PS2Keyboard, ScancodeSet2, layouts::Us104Key,
};
pub const RAW_CAPACITY: usize = 256;
pub const RAW_PER_IRQ: usize = 32;
pub const RAW_PER_TURN: usize = 64;
pub const TEXT_CAPACITY: usize = 128;
pub const COMMAND_NS: u64 = 100_000_000;
pub const DEVICE_RESET_NS: u64 = 1_000_000_000;
pub const INITIALIZE_NS: u64 = 3_000_000_000;
pub const STEP_POLLS: u64 = 100_000;
pub const RESENDS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Port {
    Keyboard,
    Mouse,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Raw {
    pub port: Port,
    pub byte: u8,
    pub errors: u8,
}
pub struct RawQueue {
    queue: crate::runtime_contract::BoundedQueue<Raw, RAW_CAPACITY>,
    lost: bool,
    received: u64,
    saturated: bool,
}
pub struct RawBatch {
    pub items: [Option<Raw>; RAW_PER_TURN],
    pub lost: bool,
    pub received: u64,
    pub dropped: u64,
    pub saturated: bool,
    pub pending: bool,
}
impl Default for RawQueue {
    fn default() -> Self {
        Self::new()
    }
}
impl RawQueue {
    pub const fn new() -> Self {
        Self {
            queue: crate::runtime_contract::BoundedQueue::new(),
            lost: false,
            received: 0,
            saturated: false,
        }
    }
    pub fn push(&mut self, raw: Raw) {
        if let Some(n) = self.received.checked_add(1) {
            self.received = n;
        } else {
            self.saturated = true;
            self.lost = true;
        }
        if self.queue.push(raw).is_err() {
            self.lost = true;
        }
    }
    pub fn drain(&mut self, paused: bool) -> RawBatch {
        let mut items = [None; RAW_PER_TURN];
        if !paused && !self.lost {
            for slot in &mut items {
                *slot = self.queue.pop();
                if slot.is_none() {
                    break;
                }
            }
        }
        let (dropped, saturated) = self.queue.rejected();
        RawBatch {
            items,
            lost: self.lost,
            received: self.received,
            dropped,
            saturated: saturated || self.saturated,
            pending: !self.queue.is_empty(),
        }
    }
    pub fn clear_after_reset(&mut self) {
        self.queue.clear();
        self.lost = false;
    }
}
/// 事务阶段先区分 AUX 与状态错误；异端口 ACK 不能完成当前命令。
pub fn transaction_byte(raw: Raw, expected: Port) -> Result<u8, &'static str> {
    if raw.errors != 0 {
        return Err("PS2_STATUS_ERROR");
    }
    if raw.port != expected {
        return Err("PS2_REPLY_PORT");
    }
    Ok(raw.byte)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Press,
    Repeat,
    Release,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub caps: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Key {
        code: KeyCode,
        phase: Phase,
        modifiers: Modifiers,
    },
    Text(char),
    PointerMove {
        dx: i16,
        dy: i16,
    },
    PointerButton {
        down: bool,
    },
    InputReset,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Controller,
    ReplyInStream,
    ScanCode,
    MouseSync,
    MouseOverflow,
}
impl Error {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Controller => "INPUT_CONTROLLER",
            Self::ReplyInStream => "INPUT_REPLY_IN_STREAM",
            Self::ScanCode => "INPUT_SCAN_CODE",
            Self::MouseSync => "INPUT_MOUSE_SYNC",
            Self::MouseOverflow => "INPUT_MOUSE_OVERFLOW",
        }
    }
}
pub struct Decoder {
    keyboard: PS2Keyboard<Us104Key, ScancodeSet2>,
    pressed: [bool; 256],
    mouse: [u8; 3],
    mouse_length: usize,
    left: bool,
}
impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}
impl Decoder {
    pub fn new() -> Self {
        Self {
            keyboard: PS2Keyboard::new(ScancodeSet2::new(), Us104Key, HandleControl::Ignore),
            pressed: [false; 256],
            mouse: [0; 3],
            mouse_length: 0,
            left: false,
        }
    }
    pub fn modifiers(&self) -> Modifiers {
        let m = self.keyboard.get_modifiers();
        Modifiers {
            shift: m.lshift || m.rshift,
            control: m.lctrl || m.rctrl || m.rctrl2,
            alt: m.lalt || m.ralt,
            caps: m.capslock,
        }
    }
    pub fn feed(&mut self, raw: Raw) -> Result<[Option<Event>; 2], Error> {
        if raw.errors != 0 {
            return Err(Error::Controller);
        }
        match raw.port {
            Port::Keyboard => self.key(raw.byte),
            Port::Mouse => self.mouse(raw.byte),
        }
    }
    fn key(&mut self, byte: u8) -> Result<[Option<Event>; 2], Error> {
        if [0xfa, 0xfe, 0xaa, 0xfc, 0xfd, 0x00, 0xff].contains(&byte) {
            return Err(Error::ReplyInStream);
        }
        let Some(key) = self.keyboard.add_byte(byte).map_err(|_| Error::ScanCode)? else {
            return Ok([None, None]);
        };
        let code = key.code;
        let phase = match key.state {
            KeyState::Up => Phase::Release,
            KeyState::Down if self.pressed[code as usize] => Phase::Repeat,
            _ => Phase::Press,
        };
        self.pressed[code as usize] = key.state == KeyState::Down;
        // 重复 Caps / Num make 不能反复翻转锁定状态。
        let decoded =
            if phase == Phase::Repeat && matches!(code, KeyCode::CapsLock | KeyCode::NumpadLock) {
                None
            } else {
                self.keyboard.process_keyevent(key)
            };
        let modifiers = self.modifiers();
        let text = match decoded {
            Some(DecodedKey::Unicode(c))
                if phase != Phase::Release
                    && !modifiers.control
                    && !modifiers.alt
                    && !c.is_control() =>
            {
                Some(Event::Text(c))
            }
            _ => None,
        };
        Ok([
            Some(Event::Key {
                code,
                phase,
                modifiers,
            }),
            text,
        ])
    }
    fn mouse(&mut self, byte: u8) -> Result<[Option<Event>; 2], Error> {
        if self.mouse_length == 0 && byte & 8 == 0 {
            return Err(Error::MouseSync);
        }
        self.mouse[self.mouse_length] = byte;
        self.mouse_length += 1;
        if self.mouse_length < 3 {
            return Ok([None, None]);
        }
        self.mouse_length = 0;
        let [flags, x, y] = self.mouse;
        if flags & 0xc0 != 0 {
            return Err(Error::MouseOverflow);
        }
        let dx = i16::from(x) - if flags & 0x10 != 0 { 256 } else { 0 };
        let dy = -(i16::from(y) - if flags & 0x20 != 0 { 256 } else { 0 });
        let down = flags & 1 != 0;
        let movement = (dx != 0 || dy != 0).then_some(Event::PointerMove { dx, dy });
        let button = (down != self.left).then_some(Event::PointerButton { down });
        self.left = down;
        Ok([movement, button])
    }
}

#[cfg(test)]
mod tests;
