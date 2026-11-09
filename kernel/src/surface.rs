//! 规范 RGB 后台与设备像素格式的纯规则，不拥有设备地址或物理页。
use crate::boot_contract::Framebuffer;
use core::convert::Infallible;
use embedded_graphics::{pixelcolor::Rgb888, prelude::*};

pub const MAX_PIXELS: usize = 4_194_304;
pub const SENTINEL_BYTES: usize = 4096;
pub const SENTINEL: u8 = 0xa7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Dimensions,
    Capacity,
    Format,
    Pitch,
    Range,
    Sentinel,
}
impl Error {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Dimensions => "SURFACE_DIMENSIONS",
            Self::Capacity => "SURFACE_CAPACITY",
            Self::Format => "SURFACE_FORMAT",
            Self::Pitch => "SURFACE_PITCH",
            Self::Range => "SURFACE_RANGE",
            Self::Sentinel => "SURFACE_SENTINEL",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    width: u32,
    height: u32,
    pixels: usize,
}
impl Geometry {
    pub fn new(width: u64, height: u64) -> Result<Self, Error> {
        if !(320..=i32::MAX as u64).contains(&width) || !(240..=i32::MAX as u64).contains(&height) {
            return Err(Error::Dimensions);
        }
        let pixels = width.checked_mul(height).ok_or(Error::Range)?;
        if pixels > MAX_PIXELS as u64 {
            return Err(Error::Capacity);
        }
        Ok(Self {
            width: width as u32,
            height: height as u32,
            pixels: pixels as usize,
        })
    }
    pub const fn width(self) -> u32 {
        self.width
    }
    pub const fn height(self) -> u32 {
        self.height
    }
    pub const fn pixels(self) -> usize {
        self.pixels
    }
    pub const fn bytes(self) -> usize {
        self.pixels * 4
    }
    pub fn allocation_bytes(self) -> usize {
        self.bytes().div_ceil(4096) * 4096 + 2 * SENTINEL_BYTES
    }
    pub const fn size(self) -> Size {
        Size::new(self.width, self.height)
    }
    pub fn index(self, p: Point) -> Option<usize> {
        (p.x >= 0 && p.y >= 0 && p.x < self.width as i32 && p.y < self.height as i32)
            .then(|| p.y as usize * self.width as usize + p.x as usize)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Format {
    pub geometry: Geometry,
    pub pitch: usize,
    pub length: usize,
    pub shifts: [u8; 3],
}
impl Format {
    pub fn new(fb: Framebuffer) -> Result<Self, Error> {
        let geometry = Geometry::new(fb.width, fb.height)?;
        if fb.bpp != 32 || fb.model != 1 {
            return Err(Error::Format);
        }
        let mut mask = 0u32;
        let mut shifts = [0; 3];
        for (index, (bits, shift)) in fb.masks.into_iter().enumerate() {
            if bits != 8 || shift > 24 {
                return Err(Error::Format);
            }
            let next = 255u32 << shift;
            if mask & next != 0 {
                return Err(Error::Format);
            }
            mask |= next;
            shifts[index] = shift;
        }
        let pitch = usize::try_from(fb.pitch).map_err(|_| Error::Range)?;
        if pitch < geometry.width as usize * 4 {
            return Err(Error::Pitch);
        }
        let length = pitch
            .checked_mul(geometry.height as usize)
            .ok_or(Error::Range)?;
        fb.physical.checked_add(length as u64).ok_or(Error::Range)?;
        if length > isize::MAX as usize {
            return Err(Error::Range);
        }
        Ok(Self {
            geometry,
            pitch,
            length,
            shifts,
        })
    }
    pub fn pack(self, canonical: u32) -> [u8; 4] {
        let r = (canonical >> 16) & 255;
        let g = (canonical >> 8) & 255;
        let b = canonical & 255;
        ((r << self.shifts[0]) | (g << self.shifts[1]) | (b << self.shifts[2])).to_le_bytes()
    }
    /// RAM 检验入口；设备 present 使用相同 pack，但只经 volatile 写设备映射。
    pub fn present(self, pixels: &[u32], destination: &mut [u8]) -> Result<(), Error> {
        if pixels.len() != self.geometry.pixels || destination.len() < self.length {
            return Err(Error::Capacity);
        }
        for (y, row) in pixels
            .chunks_exact(self.geometry.width as usize)
            .enumerate()
        {
            for (x, pixel) in row.iter().enumerate() {
                let offset = y * self.pitch + x * 4;
                destination[offset..offset + 4].copy_from_slice(&self.pack(*pixel));
            }
        }
        Ok(())
    }
}

/// 借用独占后台，不能复制所有权；所有坐标先检查再转换无符号数。
pub struct Surface<'a> {
    geometry: Geometry,
    pixels: &'a mut [u32],
}
impl<'a> Surface<'a> {
    pub fn new(geometry: Geometry, pixels: &'a mut [u32]) -> Result<Self, Error> {
        if pixels.len() != geometry.pixels {
            return Err(Error::Capacity);
        }
        Ok(Self { geometry, pixels })
    }
    pub fn pixels(&self) -> &[u32] {
        self.pixels
    }
    pub const fn geometry(&self) -> Geometry {
        self.geometry
    }
    pub fn pixel(&mut self, point: Point, color: Rgb888) {
        if let Some(index) = self.geometry.index(point) {
            self.pixels[index] = rgb(color);
        }
    }
    /// i64 端点避免 i32 + u32 溢出；遍历范围仅限实际画面。
    pub fn rectangle(&mut self, point: Point, size: Size, color: Rgb888) {
        let x0 = i64::from(point.x)
            .max(0)
            .min(i64::from(self.geometry.width));
        let y0 = i64::from(point.y)
            .max(0)
            .min(i64::from(self.geometry.height));
        let x1 = (i64::from(point.x) + i64::from(size.width))
            .max(0)
            .min(i64::from(self.geometry.width));
        let y1 = (i64::from(point.y) + i64::from(size.height))
            .max(0)
            .min(i64::from(self.geometry.height));
        for y in y0..y1 {
            let row = y as usize * self.geometry.width as usize;
            self.pixels[row + x0 as usize..row + x1 as usize].fill(rgb(color));
        }
    }
}
pub fn rgb(color: Rgb888) -> u32 {
    (color.r() as u32) << 16 | (color.g() as u32) << 8 | color.b() as u32
}
/// 绘制边界验收图元；仅由明确的 boot-test / 宿主快照验证入口调用。
pub fn boundary_probe(surface: &mut Surface<'_>) {
    let g = surface.geometry();
    surface.rectangle(
        Point::new(-4, g.height() as i32 / 2),
        Size::new(8, 4),
        Rgb888::RED,
    );
    surface.rectangle(
        Point::new(g.width() as i32 - 4, -4),
        Size::new(8, 8),
        Rgb888::GREEN,
    );
    surface.rectangle(
        Point::new(g.width() as i32 - 4, g.height() as i32 - 4),
        Size::new(u32::MAX, u32::MAX),
        Rgb888::BLUE,
    );
    surface.pixel(Point::new(i32::MIN, i32::MAX), Rgb888::WHITE);
}
impl OriginDimensions for Surface<'_> {
    fn size(&self) -> Size {
        self.geometry.size()
    }
}
impl DrawTarget for Surface<'_> {
    type Color = Rgb888;
    type Error = Infallible;
    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(p, c) in pixels {
            self.pixel(p, c);
        }
        Ok(())
    }
    fn clear(&mut self, color: Rgb888) -> Result<(), Self::Error> {
        self.pixels.fill(rgb(color));
        Ok(())
    }
    fn fill_solid(
        &mut self,
        area: &embedded_graphics::primitives::Rectangle,
        color: Rgb888,
    ) -> Result<(), Self::Error> {
        self.rectangle(area.top_left, area.size, color);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
