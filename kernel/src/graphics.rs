//! 主循环中的图形 / 输入接入；同一轮最多一次完整 present，IRQ 不进入本模块。
use crate::{
    boot::Facts,
    clock, heap, interrupts,
    platform_memory::{self, Display},
    ps2,
    serial::Serial,
};
use core::fmt::Write;
use radishroot_kernel::{
    input::{Decoder, Event, Port, Raw},
    input_view::{Status, View},
    runtime_contract::{EVENTS_PER_TURN, Queue},
};

pub struct Graphics {
    display: Display,
    view: View,
    decoder: Decoder,
    events: Queue<Event>,
    recovered: bool,
    offline: bool,
    mode: &'static str,
    injected: bool,
    paused: bool,
    raw_received: u64,
    raw_dropped: u64,
    last_report: u64,
    initial_frames: u64,
    initial_heap: u64,
}
impl Graphics {
    pub fn new(serial: &mut Serial, facts: &Facts) -> Result<Self, &'static str> {
        let _ = writeln!(
            serial,
            "RR:T07:ENTER case={}",
            if facts.mode.is_empty() {
                "normal"
            } else {
                facts.mode
            }
        );
        if !facts.mode.is_empty() {
            let _ = writeln!(serial, "RR:T07:INJECT case={}", facts.mode);
        }
        let before = platform_memory::stats().allocated_bytes;
        let display = match Display::new(
            facts,
            facts.mode == "t07-surface-allocation",
            facts.mode == "t07-surface-description",
        ) {
            Ok(display) => display,
            Err(code) => {
                let _ = writeln!(
                    serial,
                    "RR:T07:SURFACE_REJECT code={code} writes=0 before={before} after={}",
                    platform_memory::stats().allocated_bytes
                );
                return Err(code);
            }
        };
        let mut view = View::new(display.format.geometry);
        view.pitch = display.format.pitch;
        view.shifts = display.format.shifts;
        let mut state = Self {
            display,
            view,
            decoder: Decoder::new(),
            events: Queue::new(),
            recovered: false,
            offline: false,
            mode: facts.mode,
            injected: false,
            paused: false,
            raw_received: 0,
            raw_dropped: 0,
            last_report: 0,
            initial_frames: platform_memory::stats().allocated_bytes,
            initial_heap: heap::stats().allocated_bytes,
        };
        let _ = writeln!(
            serial,
            "RR:T07:SURFACE width={} height={} pitch={} pixels_bytes={} page_bytes={} allocated_before={before} allocated_after={} heap={} red={} green={} blue={}",
            state.display.format.geometry.width(),
            state.display.format.geometry.height(),
            state.display.format.pitch,
            state.display.format.geometry.bytes(),
            state.display.page_bytes(),
            state.initial_frames,
            state.initial_heap,
            state.display.format.shifts[0],
            state.display.format.shifts[1],
            state.display.format.shifts[2]
        );
        if let Err(error) = ps2::initialize(facts.mode == "t07-ps2-timeout") {
            state.view.unavailable();
            state.draw(serial)?;
            let _ = writeln!(
                serial,
                "RR:T07:PS2_FAIL code={} stage={} port={:?} irq_masked=1",
                error.code, error.stage, error.port
            );
            return Err(error.code);
        }
        let _ = writeln!(
            serial,
            "RR:T07:PS2 keyboard_set=2 translation=0 mouse_id=0 packet_bytes=3 scale=1 irq_keyboard=1 irq_mouse=12"
        );
        state.draw(serial)?;
        let _ = writeln!(
            serial,
            "RR:T07:READY case={} generation={}",
            if facts.mode.is_empty() {
                "normal"
            } else {
                facts.mode
            },
            state.display.presents
        );
        Ok(state)
    }
    fn reset(&mut self, serial: &mut Serial, reason: &'static str) {
        self.events.clear();
        self.decoder = Decoder::new();
        self.view.apply(Event::InputReset);
        let _ = writeln!(
            serial,
            "RR:T07:RESET reason={reason} raw_received={} raw_dropped={} event_dropped={} modifiers=0 left=0 text_bytes={} count={}",
            self.raw_received,
            self.raw_dropped,
            self.events.rejected().0,
            self.view.text().len(),
            self.view.resets
        );
        if self.recovered {
            interrupts::input_enabled(false);
            self.offline = true;
            self.view.unavailable();
            let _ = writeln!(serial, "RR:T07:OFFLINE reason=RECOVERY_LIMIT irq_masked=1");
            return;
        }
        self.recovered = true;
        match ps2::initialize(false) {
            Ok(()) => {
                let _ = writeln!(serial, "RR:T07:RECOVERED count=1 modifiers=0 left=0");
            }
            Err(error) => {
                self.offline = true;
                self.view.unavailable();
                let _ = writeln!(
                    serial,
                    "RR:T07:OFFLINE reason={} stage={} port={:?} irq_masked=1",
                    error.code, error.stage, error.port
                );
            }
        }
    }
    pub fn turn(&mut self, serial: &mut Serial) -> Result<bool, &'static str> {
        let mut pending = false;
        if !self.offline {
            if self.mode == "t07-input-overflow" && self.view.events > 0 && !self.injected {
                self.paused = true;
                self.injected = true;
                let _ = writeln!(
                    serial,
                    "RR:T07:PAUSE raw_consumption=1 software_capacity=256"
                );
            }
            let batch = ps2::drain(self.paused);
            self.raw_received = batch.received;
            self.raw_dropped = batch.dropped;
            pending = batch.pending;
            if batch.lost || batch.saturated {
                self.paused = false;
                self.reset(serial, "RAW_QUEUE_LOSS");
            } else {
                let mut error = None;
                for raw in batch.items.into_iter().flatten() {
                    match self.decoder.feed(raw) {
                        Ok(events) => {
                            for event in events.into_iter().flatten() {
                                if self.events.push(event).is_err() {
                                    error = Some("EVENT_QUEUE_LOSS");
                                    break;
                                }
                            }
                        }
                        Err(e) => error = Some(e.code()),
                    }
                    if error.is_some() {
                        break;
                    }
                }
                if let Some(error) = error {
                    self.reset(serial, error);
                }
            }
            for _ in 0..EVENTS_PER_TURN {
                let Some(event) = self.events.pop() else {
                    break;
                };
                self.view.apply(event);
            }
            if self.mode == "t07-corrupt-input" && self.view.events > 0 && !self.injected {
                self.injected = true;
                let _ = writeln!(
                    serial,
                    "RR:T07:CORRUPT boundary=raw source=boot-test byte=0"
                );
                let error = self
                    .decoder
                    .feed(Raw {
                        port: Port::Keyboard,
                        byte: 0,
                        errors: 0,
                    })
                    .expect_err("illegal stream injection");
                self.reset(serial, error.code());
            }
        }
        if self.view.dirty() {
            self.draw(serial)?;
        }
        let now = clock::now().map_err(|e| e.code())?;
        if now.saturating_sub(self.last_report) >= 250_000_000 {
            self.last_report = now;
            let _ = writeln!(
                serial,
                "RR:T07:IDLE generation={} events={} raw_received={} raw_dropped={} ticks={} frame_bytes={} heap={} pending={}",
                self.display.presents,
                self.view.events,
                self.raw_received,
                self.raw_dropped,
                interrupts::ticks().0,
                platform_memory::stats().allocated_bytes,
                heap::stats().allocated_bytes,
                u8::from(pending || !self.events.is_empty())
            );
        }
        Ok(pending || !self.events.is_empty())
    }
    fn draw(&mut self, serial: &mut Serial) -> Result<(), &'static str> {
        let start = clock::now().map_err(|e| e.code())?;
        let clipping = self.mode == "t07-clipping";
        {
            let mut surface = self.display.surface();
            self.view.render(&mut surface);
            if clipping {
                radishroot_kernel::surface::boundary_probe(&mut surface);
            }
        }
        self.display.present()?;
        let end = clock::now().map_err(|e| e.code())?;
        let frames = platform_memory::stats().allocated_bytes;
        let heap = heap::stats().allocated_bytes;
        if frames != self.initial_frames || heap != self.initial_heap {
            return Err("SURFACE_ACCOUNTING_CHANGED");
        }
        let status = match self.view.status {
            Status::Ready => "ready",
            Status::TextFull => "full",
            Status::InputReset => "reset",
            Status::InputUnavailable => "offline",
        };
        let f = self.display.format;
        let v = &self.view;
        let _ = write!(
            serial,
            "RR:T07:FRAME generation={} width={} height={} pitch={} red={} green={} blue={} x={} y={} left={} focus={} activate0={} activate1={} shift={} ctrl={} alt={} caps={} status={status} button_presses={} button_releases={} events={} releases={} resets={} raw_received={} raw_dropped={} event_dropped={} ticks={} elapsed_ns={} pixel_bytes={} page_bytes={} frame_bytes={} heap={} sentinel=1 clipping={} text=",
            self.display.presents,
            f.geometry.width(),
            f.geometry.height(),
            f.pitch,
            f.shifts[0],
            f.shifts[1],
            f.shifts[2],
            v.pointer.x,
            v.pointer.y,
            u8::from(v.left),
            v.focus,
            v.activations[0],
            v.activations[1],
            u8::from(v.modifiers.shift),
            u8::from(v.modifiers.control),
            u8::from(v.modifiers.alt),
            u8::from(v.modifiers.caps),
            v.button_presses,
            v.button_releases,
            v.events,
            v.releases,
            v.resets,
            self.raw_received,
            self.raw_dropped,
            self.events.rejected().0,
            interrupts::ticks().0,
            end - start,
            f.geometry.bytes(),
            self.display.page_bytes(),
            frames,
            heap,
            u8::from(clipping)
        );
        if v.text().is_empty() {
            let _ = write!(serial, "-");
        } else {
            for byte in v.text().bytes() {
                let _ = write!(serial, "{byte:02x}");
            }
        }
        let _ = writeln!(serial);
        Ok(())
    }
}
