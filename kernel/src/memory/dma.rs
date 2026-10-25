//! DMA 生命周期纯契约。完成 / 停止证据只能由设备适配层确认，超时不是证据。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Prepared,
    InFlight,
    Completed,
    Quarantined,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Direction {
    ToDevice,
    FromDevice,
    Both,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Error {
    Range,
    Full,
    Unknown,
    State,
    Mismatch,
    DeviceBusy,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Token {
    slot: usize,
    generation: u64,
}
#[derive(Clone, Copy, Debug)]
struct Entry {
    generation: u64,
    physical: u64,
    original: usize,
    length: usize,
    direction: Direction,
    state: State,
}
pub struct Ledger {
    entries: [Option<Entry>; 64],
    next: u64,
}
impl Default for Ledger {
    fn default() -> Self {
        Self::new()
    }
}
impl Ledger {
    pub const fn new() -> Self {
        Self {
            entries: [None; 64],
            next: 1,
        }
    }
    pub fn share(
        &mut self,
        physical: u64,
        original: usize,
        length: usize,
        direction: Direction,
    ) -> Result<Token, Error> {
        if physical == 0
            || original == 0
            || length == 0
            || physical.checked_add(length as u64).is_none()
            || original.checked_add(length).is_none()
        {
            return Err(Error::Range);
        }
        // 同时共享重叠内存会破坏所有权；包括 quarantine，不能绕过旧请求。
        for e in self.entries.iter().flatten() {
            if (physical < e.physical + e.length as u64 && e.physical < physical + length as u64)
                || (original < e.original + e.length && e.original < original + length)
            {
                return Err(Error::Mismatch);
            }
        }
        let slot = self
            .entries
            .iter()
            .position(Option::is_none)
            .ok_or(Error::Full)?;
        let generation = self.next;
        self.next = self.next.checked_add(1).ok_or(Error::Full)?;
        self.entries[slot] = Some(Entry {
            generation,
            physical,
            original,
            length,
            direction,
            state: State::Prepared,
        });
        Ok(Token { slot, generation })
    }
    fn entry(&mut self, token: Token) -> Result<&mut Entry, Error> {
        self.entries
            .get_mut(token.slot)
            .and_then(Option::as_mut)
            .filter(|e| e.generation == token.generation)
            .ok_or(Error::Unknown)
    }
    pub fn submit(&mut self, token: Token) -> Result<(), Error> {
        let e = self.entry(token)?;
        if e.state != State::Prepared {
            return Err(Error::State);
        }
        e.state = State::InFlight;
        Ok(())
    }
    /// 调用方必须已经从设备完成队列读到本 token 的完成记录。
    pub fn complete(&mut self, token: Token) -> Result<(), Error> {
        let e = self.entry(token)?;
        if e.state != State::InFlight {
            return Err(Error::State);
        }
        e.state = State::Completed;
        Ok(())
    }
    pub fn timeout(&mut self, token: Token) -> Result<(), Error> {
        let e = self.entry(token)?;
        if e.state != State::InFlight {
            return Err(Error::State);
        }
        e.state = State::Quarantined;
        Ok(())
    }
    /// 设备适配层必须先验证复位完成、停止 DMA，不能只关闭 CPU 的轮询。
    pub fn stopped(&mut self, token: Token) -> Result<(), Error> {
        let e = self.entry(token)?;
        if !matches!(e.state, State::InFlight | State::Quarantined) {
            return Err(Error::State);
        }
        e.state = State::Completed;
        Ok(())
    }
    pub fn identify(
        &self,
        physical: u64,
        original: usize,
        length: usize,
        direction: Direction,
    ) -> Result<Token, Error> {
        let (slot, e) = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| e.as_ref().map(|e| (i, e)))
            .find(|(_, e)| e.physical == physical)
            .ok_or(Error::Unknown)?;
        if e.original != original || e.length != length || e.direction != direction {
            return Err(Error::Mismatch);
        }
        Ok(Token {
            slot,
            generation: e.generation,
        })
    }
    pub fn release(&mut self, token: Token) -> Result<(), Error> {
        let e = self.entry(token)?;
        if !matches!(e.state, State::Prepared | State::Completed) {
            return Err(Error::DeviceBusy);
        }
        self.entries[token.slot] = None;
        Ok(())
    }
    pub fn state(&mut self, token: Token) -> Result<State, Error> {
        Ok(self.entry(token)?.state)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timeout_cannot_release_or_reuse() {
        let mut l = Ledger::new();
        let t = l
            .share(0x1000, 0x10000, 512, Direction::FromDevice)
            .unwrap();
        l.submit(t).unwrap();
        assert_eq!(l.release(t), Err(Error::DeviceBusy));
        l.timeout(t).unwrap();
        assert_eq!(l.release(t), Err(Error::DeviceBusy));
        assert_eq!(l.complete(t), Err(Error::State));
        assert_eq!(
            l.share(0x1000, 0x20000, 512, Direction::Both),
            Err(Error::Mismatch)
        );
        l.stopped(t).unwrap();
        l.release(t).unwrap();
        assert_eq!(l.release(t), Err(Error::Unknown));
        let t2 = l
            .share(0x1000, 0x10000, 512, Direction::FromDevice)
            .unwrap();
        assert_ne!(t, t2);
        assert_eq!(l.submit(t), Err(Error::Unknown));
    }
    #[test]
    fn completion_and_cancel_before_submit_are_valid() {
        let mut l = Ledger::new();
        let t = l.share(0x1000, 0x10000, 512, Direction::ToDevice).unwrap();
        assert_eq!(
            l.identify(0x1000, 0x10000, 513, Direction::ToDevice),
            Err(Error::Mismatch)
        );
        assert_eq!(
            l.identify(0x1000, 0x10000, 512, Direction::FromDevice),
            Err(Error::Mismatch)
        );
        l.submit(t).unwrap();
        l.complete(t).unwrap();
        l.release(t).unwrap();
        let t = l.share(0x2000, 0x20000, 1, Direction::Both).unwrap();
        l.release(t).unwrap();
        assert_eq!(l.share(u64::MAX, 1, 2, Direction::Both), Err(Error::Range));
    }
}
