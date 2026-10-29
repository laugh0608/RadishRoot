//! 原始 SDL 事件保留 Esc 和 FocusLost；simulator 的事件封装会把 Esc 当作 Quit。
use crate::{Result, render, save};
use radishroot_kernel::{
    input::{Decoder, Event, Port, Raw},
    input_view::View,
    surface::Geometry,
};
use sdl2::{
    event::{Event as SdlEvent, WindowEvent},
    keyboard::Scancode,
    mouse::MouseButton,
    pixels::PixelFormatEnum,
};
use std::{
    fs::OpenOptions,
    io::Write,
    path::Path,
    thread,
    time::{Duration, Instant},
};

pub fn run(output: &Path) -> Result<()> {
    let mut log = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("gui-events.txt"))?;
    let context = sdl2::init()?;
    let video = context.video()?;
    let window = video
        .window("RadishRoot T07 input and display", 1024, 768)
        .position_centered()
        .build()?;
    let mut canvas = window.into_canvas().software().build()?;
    let creator = canvas.texture_creator();
    let mut texture = creator.create_texture_streaming(PixelFormatEnum::RGB24, 1024, 768)?;
    let mut pump = context.event_pump()?;
    let mut view = View::new(Geometry::new(1024, 768).map_err(|e| e.code())?);
    let mut decoder = Decoder::new();
    let started = Instant::now();
    let mut generation = 0u64;
    writeln!(
        log,
        "pid={} global_grab=off timeout_seconds=600",
        std::process::id()
    )?;
    loop {
        for event in pump.poll_iter() {
            match event {
                SdlEvent::Quit { .. } => {
                    save(&mut view, output, "gui-final")?;
                    writeln!(
                        log,
                        "exit=window-close elapsed_ms={}",
                        started.elapsed().as_millis()
                    )?;
                    return Ok(());
                }
                SdlEvent::Window {
                    win_event: WindowEvent::FocusLost,
                    ..
                } => {
                    decoder = Decoder::new();
                    view.apply(Event::InputReset);
                    writeln!(log, "event=focus-lost reset=1")?;
                }
                SdlEvent::Window {
                    win_event: WindowEvent::FocusGained,
                    ..
                } => {
                    writeln!(log, "event=focus-gained")?;
                }
                SdlEvent::KeyDown {
                    scancode: Some(code),
                    ..
                }
                | SdlEvent::KeyUp {
                    scancode: Some(code),
                    ..
                } => {
                    let release = matches!(event, SdlEvent::KeyUp { .. });
                    if let Some((extended, byte)) = scan_code(code) {
                        let mut bytes = [0; 3];
                        let mut length = 0;
                        if extended {
                            bytes[length] = 0xe0;
                            length += 1;
                        }
                        if release {
                            bytes[length] = 0xf0;
                            length += 1;
                        }
                        bytes[length] = byte;
                        length += 1;
                        for byte in &bytes[..length] {
                            match decoder.feed(Raw {
                                port: Port::Keyboard,
                                byte: *byte,
                                errors: 0,
                            }) {
                                Ok(events) => {
                                    for event in events.into_iter().flatten() {
                                        writeln!(log, "event={event:?}")?;
                                        view.apply(event);
                                    }
                                }
                                Err(error) => {
                                    writeln!(log, "decode-error={error:?}")?;
                                    decoder = Decoder::new();
                                    view.apply(Event::InputReset);
                                }
                            }
                        }
                    } else {
                        writeln!(log, "unsupported-scancode={code:?}")?;
                    }
                }
                SdlEvent::MouseMotion { xrel, yrel, .. } => {
                    let dx = xrel.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
                    let dy = yrel.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
                    view.apply(Event::PointerMove { dx, dy });
                    writeln!(log, "event=relative-pointer dx={dx} dy={dy}")?;
                }
                SdlEvent::MouseButtonDown {
                    mouse_btn: MouseButton::Left,
                    ..
                } => {
                    view.apply(Event::PointerButton { down: true });
                    writeln!(log, "event=left-down")?;
                }
                SdlEvent::MouseButtonUp {
                    mouse_btn: MouseButton::Left,
                    ..
                } => {
                    view.apply(Event::PointerButton { down: false });
                    writeln!(log, "event=left-up")?;
                }
                _ => {}
            }
        }
        if view.dirty() {
            let (_, pixels) = render(&mut view)?;
            let rgb = pixels
                .iter()
                .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8])
                .collect::<Vec<_>>();
            texture.update(None, &rgb, 1024 * 3)?;
            canvas.copy(&texture, None, None)?;
            canvas.present();
            generation += 1;
            writeln!(
                log,
                "frame={generation} events={} releases={} resets={} focus={} left={} text={:?}",
                view.events,
                view.releases,
                view.resets,
                view.focus,
                view.left,
                view.text()
            )?;
            log.flush()?;
        }
        if started.elapsed() >= Duration::from_secs(600) {
            save(&mut view, output, "gui-timeout")?;
            return Err("GUI time budget reached".into());
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn scan_code(code: Scancode) -> Option<(bool, u8)> {
    use Scancode::*;
    Some(match code {
        A => (false, 0x1c),
        B => (false, 0x32),
        C => (false, 0x21),
        D => (false, 0x23),
        E => (false, 0x24),
        F => (false, 0x2b),
        G => (false, 0x34),
        H => (false, 0x33),
        I => (false, 0x43),
        J => (false, 0x3b),
        K => (false, 0x42),
        L => (false, 0x4b),
        M => (false, 0x3a),
        N => (false, 0x31),
        O => (false, 0x44),
        P => (false, 0x4d),
        Q => (false, 0x15),
        R => (false, 0x2d),
        S => (false, 0x1b),
        T => (false, 0x2c),
        U => (false, 0x3c),
        V => (false, 0x2a),
        W => (false, 0x1d),
        X => (false, 0x22),
        Y => (false, 0x35),
        Z => (false, 0x1a),
        Num1 => (false, 0x16),
        Num2 => (false, 0x1e),
        Num3 => (false, 0x26),
        Num4 => (false, 0x25),
        Num5 => (false, 0x2e),
        Num6 => (false, 0x36),
        Num7 => (false, 0x3d),
        Num8 => (false, 0x3e),
        Num9 => (false, 0x46),
        Num0 => (false, 0x45),
        Space => (false, 0x29),
        Return => (false, 0x5a),
        Escape => (false, 0x76),
        Backspace => (false, 0x66),
        Tab => (false, 0x0d),
        CapsLock => (false, 0x58),
        LShift => (false, 0x12),
        RShift => (false, 0x59),
        LCtrl => (false, 0x14),
        RCtrl => (true, 0x14),
        LAlt => (false, 0x11),
        RAlt => (true, 0x11),
        Minus => (false, 0x4e),
        Equals => (false, 0x55),
        LeftBracket => (false, 0x54),
        RightBracket => (false, 0x5b),
        Backslash => (false, 0x5d),
        Semicolon => (false, 0x4c),
        Apostrophe => (false, 0x52),
        Grave => (false, 0x0e),
        Comma => (false, 0x41),
        Period => (false, 0x49),
        Slash => (false, 0x4a),
        Up => (true, 0x75),
        Down => (true, 0x72),
        Left => (true, 0x6b),
        Right => (true, 0x74),
        Home => (true, 0x6c),
        End => (true, 0x69),
        PageUp => (true, 0x7d),
        PageDown => (true, 0x7a),
        Insert => (true, 0x70),
        Delete => (true, 0x71),
        F1 => (false, 0x05),
        F2 => (false, 0x06),
        F3 => (false, 0x04),
        F4 => (false, 0x0c),
        F5 => (false, 0x03),
        F6 => (false, 0x0b),
        F7 => (false, 0x83),
        F8 => (false, 0x0a),
        F9 => (false, 0x01),
        F10 => (false, 0x09),
        F11 => (false, 0x78),
        F12 => (false, 0x07),
        NumLockClear => (false, 0x77),
        KpDivide => (true, 0x4a),
        KpMultiply => (false, 0x7c),
        KpMinus => (false, 0x7b),
        KpPlus => (false, 0x79),
        KpEnter => (true, 0x5a),
        Kp0 => (false, 0x70),
        Kp1 => (false, 0x69),
        Kp2 => (false, 0x72),
        Kp3 => (false, 0x7a),
        Kp4 => (false, 0x6b),
        Kp5 => (false, 0x73),
        Kp6 => (false, 0x74),
        Kp7 => (false, 0x6c),
        Kp8 => (false, 0x75),
        Kp9 => (false, 0x7d),
        KpPeriod => (false, 0x71),
        _ => return None,
    })
}
