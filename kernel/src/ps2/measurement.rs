//! 只在测量模式采集 QMP 单键按下的 i8042 到达时间。
use crate::{clock, exclusive::Exclusive};
use radishroot_kernel::input::{Port, Raw, RawBatch};
use x86_64::instructions::interrupts;

#[derive(Clone, Copy)]
pub struct Stamp {
    pub byte: u8,
    pub received_ns: u64,
}

#[derive(Default)]
struct Capture {
    pressed: Option<u8>,
    breaking: bool,
    pending: Option<Stamp>,
    fault: Option<&'static str>,
}
static STATE: Exclusive<Option<Capture>> = Exclusive::new(None);

pub fn enable() {
    *STATE.lock() = Some(Capture::default());
}

pub(super) fn capture(raw: Raw) {
    let mut state = STATE.lock();
    let Some(state) = state.as_mut().filter(|state| state.fault.is_none()) else {
        return;
    };
    if raw.port != Port::Keyboard || raw.errors != 0 {
        state.fault = Some("M1_INPUT_DEVICE");
        return;
    }
    match (state.pressed, state.breaking, raw.byte) {
        (None, false, code @ (0x1c | 0x32)) if state.pending.is_none() => {
            // 单 BSP 上时钟 guard 禁中断，取样不会打断持有同一锁的主循环。
            match clock::now() {
                Ok(received_ns) => {
                    state.pressed = Some(code);
                    state.pending = Some(Stamp {
                        byte: if code == 0x1c { b'a' } else { b'b' },
                        received_ns,
                    });
                }
                Err(error) => state.fault = Some(error.code()),
            }
        }
        (Some(_), false, 0xf0) => state.breaking = true,
        (Some(expected), true, code) if code == expected => {
            state.pressed = None;
            state.breaking = false;
        }
        _ => state.fault = Some("M1_INPUT_SEQUENCE"),
    }
}

pub fn drain() -> Result<(RawBatch, Option<Stamp>), &'static str> {
    // 原始字节与时间戳一同交接，随后到达的输入属于下一轮。
    interrupts::without_interrupts(|| {
        let batch = super::drain(false);
        let mut guard = STATE.lock();
        let state = guard.as_mut().ok_or("M1_INPUT_DISABLED")?;
        if let Some(fault) = state.fault {
            return Err(fault);
        }
        if batch.lost || batch.saturated || batch.pending {
            return Err("M1_INPUT_BATCH");
        }
        Ok((batch, state.pending.take()))
    })
}
