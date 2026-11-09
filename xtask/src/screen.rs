//! 独立 PPM / 像素期望：不依赖 guest 绘图库，不用 guest 自报 CRC 代替扫描输出。
use crate::{
    error::{Error, Result},
    process::{self, Runner},
    vm::{fields, number},
};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn bad(message: impl Into<String>) -> Error {
    Error::new("screen", message)
}
pub struct Ppm {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}
impl Ppm {
    pub fn read(path: &Path) -> Result<Self> {
        let length = fs::metadata(path)?.len();
        if length > 16 * 1024 * 1024 {
            return Err(bad("PPM exceeds capacity"));
        }
        Self::parse(&fs::read(path)?)
    }
    fn parse(bytes: &[u8]) -> Result<Self> {
        let mut position = 0;
        let mut tokens = Vec::new();
        for _ in 0..4 {
            while bytes.get(position).is_some_and(u8::is_ascii_whitespace) {
                position += 1;
            }
            let start = position;
            while bytes
                .get(position)
                .is_some_and(|b| !b.is_ascii_whitespace())
            {
                position += 1;
            }
            tokens.push(
                std::str::from_utf8(&bytes[start..position])
                    .map_err(|_| bad("PPM header UTF-8"))?,
            );
        }
        if tokens[0] != "P6"
            || tokens[3] != "255"
            || !bytes.get(position).is_some_and(u8::is_ascii_whitespace)
        {
            return Err(bad("expected binary RGB PPM"));
        }
        // 只消耗 header 的一个分隔符；像素第一个字节可能本身就是空白。
        position += 1;
        let width = tokens[1].parse::<usize>().map_err(|_| bad("width"))?;
        let height = tokens[2].parse::<usize>().map_err(|_| bad("height"))?;
        let length = width
            .checked_mul(height)
            .and_then(|n| n.checked_mul(3))
            .filter(|n| *n <= 4_194_304 * 3 && *n > 0)
            .ok_or_else(|| bad("PPM dimensions"))?;
        if bytes.len() - position != length {
            return Err(bad("PPM pixel length differs from dimensions"));
        }
        Ok(Self {
            width,
            height,
            pixels: bytes[position..].to_vec(),
        })
    }
    fn pixel(&self, x: usize, y: usize) -> Result<[u8; 3]> {
        if x >= self.width || y >= self.height {
            return Err(bad("pixel outside image"));
        }
        let offset = (y * self.width + x) * 3;
        Ok(self.pixels[offset..offset + 3].try_into().unwrap())
    }
}
pub fn independent(image: &Ppm, frame: &str) -> Result<()> {
    let f = fields(frame)?;
    let w = number(&f, "width")? as usize;
    let h = number(&f, "height")? as usize;
    if image.width != w || image.height != h {
        return Err(bad("actual screenshot dimensions differ"));
    }
    let x = number(&f, "x")? as usize;
    let y = number(&f, "y")? as usize;
    let clipped = number(&f, "clipping")? == 1;
    let pointer_covers =
        |px: usize, py: usize| py >= y && py - y < 12 && px >= x && px - x <= (py - y) / 2;
    for (px, py, color) in [
        (0, 0, [255, 0, 0]),
        (w - 1, 0, [0, 255, 0]),
        (0, h - 1, [0, 0, 255]),
        (
            w - 1,
            h - 1,
            if clipped {
                [0, 0, 255]
            } else {
                [255, 255, 255]
            },
        ),
    ] {
        if !pointer_covers(px, py) && image.pixel(px, py)? != color {
            return Err(bad(format!("corner/color mismatch at {px},{py}")));
        }
    }
    if image.pixel(x, y)? != [32, 47, 42] {
        return Err(bad("pointer origin not visible"));
    }
    // 原始 BDF：10x20 R 第 3 行 7E00，baseline=15；6x10 R 第 1 行 F0，baseline=7。
    let (row, bits): (usize, &[u8]) = if w >= 800 && h >= 600 {
        (14, &[0, 1, 1, 1, 1, 1, 1, 0, 0, 0])
    } else {
        (15, &[1, 1, 1, 1, 0, 0])
    };
    for (i, bit) in bits.iter().enumerate() {
        let color = if *bit == 1 {
            [32, 47, 42]
        } else {
            [252, 253, 251]
        };
        if !pointer_covers(20 + i, row) && image.pixel(20 + i, row)? != color {
            return Err(bad("independent R glyph row mismatch"));
        }
    }
    for (px, color) in [
        (w - 72, [255, 0, 0]),
        (w - 54, [0, 255, 0]),
        (w - 36, [0, 0, 255]),
    ] {
        if !pointer_covers(px, 16) && image.pixel(px, 16)? != color {
            return Err(bad("RGB sample mismatch"));
        }
    }
    if clipped {
        for px in 0..4 {
            if image.pixel(px, h / 2)? != [255, 0, 0] {
                return Err(bad("negative-coordinate clip mismatch"));
            }
        }
        if image.pixel(4, h / 2)? != [239, 242, 239] {
            return Err(bad("clipping touched outside target"));
        }
    }
    Ok(())
}
pub struct Snapshot {
    pub name: String,
    pub frame: String,
    pub path: PathBuf,
}
pub fn verify(
    runner: &mut Runner,
    host: &Path,
    directory: &Path,
    snapshots: &[Snapshot],
) -> Result<()> {
    let mut report = Vec::new();
    for snapshot in snapshots {
        let image = Ppm::read(&snapshot.path)?;
        independent(&image, &snapshot.frame)?;
        let state = directory.join(format!("{}.state", snapshot.name));
        fs::write(&state, format!("{}\n", snapshot.frame))?;
        let output = directory.join(format!("{}-host", snapshot.name));
        process::directory(&output)?;
        runner.run(Command::new(host).arg(&output).arg("--state").arg(&state))?;
        let shared = Ppm::read(&output.join("frame.ppm"))?;
        if image.width != shared.width
            || image.height != shared.height
            || image.pixels != shared.pixels
        {
            let mismatch = image
                .pixels
                .iter()
                .zip(&shared.pixels)
                .position(|(a, b)| a != b);
            return Err(bad(format!(
                "host/guest pixels differ: {} first_byte={mismatch:?}",
                snapshot.name
            )));
        }
        crate::digest(&snapshot.path, runner)?;
        report.push(json!({"name":snapshot.name,"width":image.width,"height":image.height,"independent_pixels":true,"host_same_view":true,"state":snapshot.frame,"screen":snapshot.path}));
    }
    fs::write(
        directory.join("screen-report.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| bad(e.to_string()))?,
    )?;
    if let Some(snapshot) = snapshots.first() {
        let mut damaged = Ppm::read(&snapshot.path)?;
        let f = fields(&snapshot.frame)?;
        let x = number(&f, "x")? as usize;
        let y = number(&f, "y")? as usize;
        damaged.pixels[(y * damaged.width + x) * 3] ^= 1;
        if independent(&damaged, &snapshot.frame).is_ok() {
            return Err(bad("damaged actual screenshot accepted"));
        }
        runner.record("negative-screen: corrupted actual pointer pixel rejected")?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ppm_preserves_whitespace_pixels_and_rejects_truncation_overflow() {
        let good = b"P6\n1 1\n255\n\n \t";
        assert_eq!(Ppm::parse(good).unwrap().pixels, b"\n \t");
        for bad in [
            &good[..good.len() - 1],
            b"P6\n999999999999 999999999999\n255\n",
            b"P3\n1 1\n255\nabc",
        ] {
            assert!(Ppm::parse(bad).is_err());
        }
    }
}
