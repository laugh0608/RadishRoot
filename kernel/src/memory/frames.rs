//! 固定 512 MiB 物理地址上界；只分配 usable 的完整页，元数据不依赖堆。
use crate::boot_contract::{self, Region};
pub const PAGE: u64 = 4096;
pub const LIMIT: u64 = 512 * 1024 * 1024;
const WORDS: usize = (LIMIT / PAGE / 64) as usize;
const RECORDS: usize = 256;
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Error {
    Map,
    Range,
    Alignment,
    Exhausted,
    Records,
    Unknown,
    Busy,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Purpose {
    Heap,
    PageTable,
    Dma,
    Test,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Block {
    pub base: u64,
    pub pages: usize,
    id: u64,
    purpose: Purpose,
}
pub struct Frames {
    free: [u64; WORDS],
    records: [Option<Block>; RECORDS],
    next_id: u64,
    available: usize,
    tracked: usize,
    total_usable: u64,
    initialized: bool,
}
impl Default for Frames {
    fn default() -> Self {
        Self::new()
    }
}
impl Frames {
    pub const fn new() -> Self {
        Self {
            free: [0; WORDS],
            records: [None; RECORDS],
            next_id: 1,
            available: 0,
            tracked: 0,
            total_usable: 0,
            initialized: false,
        }
    }
    pub fn init(&mut self, regions: &[Region]) -> Result<(), Error> {
        if self.initialized {
            return Err(Error::Busy);
        }
        boot_contract::memory(regions).map_err(|_| Error::Map)?;
        self.total_usable = regions
            .iter()
            .filter(|r| r.kind == 0)
            .try_fold(0u64, |n, r| n.checked_add(r.length))
            .ok_or(Error::Range)?;
        for r in regions.iter().filter(|r| r.kind == 0 && r.base < LIMIT) {
            let start = r
                .base
                .max(1024 * 1024)
                .checked_add(PAGE - 1)
                .ok_or(Error::Range)?
                / PAGE;
            let end = r.base.checked_add(r.length).ok_or(Error::Range)?.min(LIMIT) / PAGE;
            for frame in start..end {
                self.free[frame as usize / 64] |= 1 << (frame % 64);
                self.available += 1;
            }
        }
        if self.available == 0 {
            return Err(Error::Exhausted);
        }
        self.initialized = true;
        self.tracked = self.available;
        Ok(())
    }
    pub fn stats(&self) -> crate::runtime_contract::MemoryStats {
        crate::runtime_contract::MemoryStats::new(
            self.total_usable,
            self.tracked as u64 * PAGE,
            (self.tracked - self.available) as u64 * PAGE,
            crate::runtime_contract::MemoryScope::UsableFrames,
        )
        .expect("frame accounting")
    }
    pub fn available(&self) -> usize {
        self.available
    }
    pub fn allocate(
        &mut self,
        pages: usize,
        align_pages: usize,
        purpose: Purpose,
    ) -> Result<Block, Error> {
        if !self.initialized || pages == 0 || pages > (LIMIT / PAGE) as usize {
            return Err(Error::Range);
        }
        if !align_pages.is_power_of_two() || align_pages > (LIMIT / PAGE) as usize {
            return Err(Error::Alignment);
        }
        let slot = self
            .records
            .iter()
            .position(Option::is_none)
            .ok_or(Error::Records)?;
        if pages > self.available {
            return Err(Error::Exhausted);
        }
        let upper = (LIMIT / PAGE) as usize;
        let mut start = 256;
        while start <= upper - pages {
            start = start.checked_add(align_pages - 1).ok_or(Error::Range)? & !(align_pages - 1);
            if start > upper - pages {
                break;
            }
            let mut count = 0;
            while count < pages
                && self.free[(start + count) / 64] & (1 << ((start + count) % 64)) != 0
            {
                count += 1;
            }
            if count == pages {
                let id = self.next_id;
                self.next_id = id.checked_add(1).ok_or(Error::Records)?;
                let block = Block {
                    base: start as u64 * PAGE,
                    pages,
                    id,
                    purpose,
                };
                for i in start..start + pages {
                    self.free[i / 64] &= !(1 << (i % 64));
                }
                self.available -= pages;
                self.records[slot] = Some(block);
                return Ok(block);
            }
            start += count + 1;
        }
        Err(Error::Exhausted)
    }
    pub fn release(&mut self, block: Block) -> Result<(), Error> {
        let slot = self
            .records
            .iter()
            .position(|item| *item == Some(block))
            .ok_or(Error::Unknown)?;
        // 全部整数和范围来自本实例创建的不可伪造字段；匹配代次后才改位图。
        let start = (block.base / PAGE) as usize;
        for i in start..start + block.pages {
            self.free[i / 64] |= 1 << (i % 64);
        }
        self.available += block.pages;
        self.records[slot] = None;
        Ok(())
    }
    pub fn contains_allocated(&self, base: u64, length: usize, purpose: Purpose) -> bool {
        let Some(end) = base.checked_add(length as u64).filter(|_| length != 0) else {
            return false;
        };
        self.records.iter().flatten().any(|b| {
            b.purpose == purpose && b.base <= base && end <= b.base + b.pages as u64 * PAGE
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stats_track_success_and_keep_failure_unchanged() {
        let mut f = frames();
        let before = f.stats();
        assert_eq!(before.total_bytes, 0x100001 + 0xbfff);
        assert_eq!(before.tracked_bytes, 11 * PAGE);
        assert_eq!(before.allocated_bytes, 0);
        let block = f.allocate(2, 1, Purpose::Test).unwrap();
        assert_eq!(f.stats().allocated_bytes, 2 * PAGE);
        let allocated = f.stats();
        assert!(f.allocate(12, 1, Purpose::Test).is_err());
        assert_eq!(f.stats(), allocated);
        f.release(block).unwrap();
        assert_eq!(f.stats(), before);
    }
    fn frames() -> Frames {
        let mut f = Frames::new();
        f.init(&[
            Region {
                base: 0,
                length: 0x100001,
                kind: 0,
            },
            Region {
                base: 0x101000,
                length: 0x3000,
                kind: 6,
            },
            Region {
                base: 0x104001,
                length: 0xbfff,
                kind: 0,
            },
        ])
        .unwrap();
        f
    }
    #[test]
    fn rounds_inward_and_preserves_reserved_ranges() {
        let mut f = frames();
        assert_eq!(f.available(), 11);
        let b = f.allocate(2, 4, Purpose::Dma).unwrap();
        assert_eq!(b.base, 0x108000);
        assert!(f.contains_allocated(b.base, 8192, Purpose::Dma));
        assert!(!f.contains_allocated(b.base, 8193, Purpose::Dma));
        assert!(!f.contains_allocated(b.base, 1, Purpose::Heap));
        f.release(b).unwrap();
        assert_eq!(f.available(), 11);
        assert_eq!(f.release(b), Err(Error::Unknown));
        let c = f.allocate(2, 4, Purpose::Dma).unwrap();
        assert_eq!(c.base, b.base);
        assert_ne!(c.id, b.id);
        assert_eq!(f.release(b), Err(Error::Unknown));
    }
    #[test]
    fn bounded_failure_does_not_consume_pages() {
        let mut f = frames();
        let n = f.available();
        for (p, a, e) in [
            (0, 1, Error::Range),
            (1, 3, Error::Alignment),
            (12, 1, Error::Exhausted),
            (usize::MAX, 1, Error::Range),
        ] {
            assert_eq!(f.allocate(p, a, Purpose::Test), Err(e));
            assert_eq!(f.available(), n);
        }
        let b = f.allocate(11, 1, Purpose::Test).unwrap();
        assert_eq!(b.base, 0x105000);
        assert_eq!(f.allocate(1, 1, Purpose::Test), Err(Error::Exhausted));
        assert_eq!(f.init(&[]), Err(Error::Busy));
    }
    #[test]
    fn rejects_bad_maps_and_ignores_unknown_types() {
        let mut f = Frames::new();
        assert_eq!(
            f.init(&[Region {
                base: u64::MAX,
                length: 4096,
                kind: 0
            }]),
            Err(Error::Map)
        );
        let mut f = Frames::new();
        assert_eq!(
            f.init(&[Region {
                base: 0x100000,
                length: 4096,
                kind: 99
            }]),
            Err(Error::Map)
        );
    }
}
