//! 串口帧快照只用于同状态宿主渲染比较，不代替真实输入与独立像素断言。
use crate::Result;
use embedded_graphics::prelude::Point;
use radishroot_kernel::{
    input::{Event, Modifiers},
    input_view::{Status, View},
    surface::Geometry,
};
use std::{collections::BTreeMap, fs, path::Path};
pub fn read(path: &Path) -> Result<(View, bool)> {
    if fs::metadata(path)?.len() > 4096 {
        return Err("frame snapshot exceeds 4096 bytes".into());
    }
    let line = fs::read_to_string(path)?;
    let mut tokens = line.split_whitespace();
    if tokens.next() != Some("RR:T07:FRAME") {
        return Err("expected frame snapshot".into());
    }
    let mut fields = BTreeMap::new();
    for token in tokens {
        let (k, v) = token.split_once('=').ok_or("invalid snapshot field")?;
        if fields.insert(k, v).is_some() {
            return Err("duplicate snapshot field".into());
        }
    }
    let number = |k| -> Result<u64> { Ok(fields.get(k).ok_or("missing snapshot field")?.parse()?) };
    let boolean = |k| -> Result<bool> {
        match number(k)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err("non-boolean snapshot".into()),
        }
    };
    let g = Geometry::new(number("width")?, number("height")?).map_err(|e| e.code())?;
    let mut view = View::new(g);
    let text = fields.get("text").ok_or("missing text")?;
    if *text != "-" {
        if text.len() > 256
            || !text.len().is_multiple_of(2)
            || !text.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("invalid text hex length".into());
        }
        let bytes = (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for c in std::str::from_utf8(&bytes)?.chars() {
            view.apply(Event::Text(c));
        }
    }
    let x = number("x")?;
    let y = number("y")?;
    let focus = number("focus")?;
    if x >= g.width() as u64 || y >= g.height() as u64 || focus > 1 {
        return Err("invalid snapshot state".into());
    }
    view.pointer = Point::new(x as i32, y as i32);
    view.focus = focus as u8;
    view.left = boolean("left")?;
    view.activations = [number("activate0")?, number("activate1")?];
    view.modifiers = Modifiers {
        shift: boolean("shift")?,
        control: boolean("ctrl")?,
        alt: boolean("alt")?,
        caps: boolean("caps")?,
    };
    view.pitch = number("pitch")?.try_into()?;
    view.shifts = [
        number("red")?.try_into()?,
        number("green")?.try_into()?,
        number("blue")?.try_into()?,
    ];
    view.status = match *fields.get("status").ok_or("missing status")? {
        "ready" => Status::Ready,
        "full" => Status::TextFull,
        "reset" => Status::InputReset,
        "offline" => Status::InputUnavailable,
        _ => return Err("invalid status".into()),
    };
    Ok((view, boolean("clipping")?))
}
