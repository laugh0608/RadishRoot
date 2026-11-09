//! 单核运行时的纯规则：时间不来自 IRQ 计数，队列与等待都有独立上界。
pub const QUEUE_CAPACITY: usize = 64;
pub const EVENTS_PER_TURN: usize = 16;
pub const REQUEST_NS: u64 = 2_000_000_000;
pub const RESET_NS: u64 = 100_000_000;
pub const REQUEST_POLLS: u64 = 2_000_000;
pub const RESET_POLLS: u64 = 100_000;
pub const STALLED_READS: u64 = 100_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeError {
    Period,
    Backwards,
    Overflow,
    Stalled,
    Inconsistent,
    Unavailable,
}
impl TimeError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Period => "CLOCK_PERIOD",
            Self::Backwards => "CLOCK_BACKWARDS",
            Self::Overflow => "CLOCK_OVERFLOW",
            Self::Stalled => "CLOCK_STALLED",
            Self::Inconsistent => "CLOCK_INCONSISTENT",
            Self::Unavailable => "CLOCK_UNAVAILABLE",
        }
    }
}
pub struct Clock {
    period_fs: u64,
    origin: u64,
    last: u64,
    repeated: u64,
}
impl Clock {
    pub fn new(period_fs: u64, counter: u64) -> Result<Self, TimeError> {
        if !(1..=100_000_000).contains(&period_fs) {
            return Err(TimeError::Period);
        }
        Ok(Self {
            period_fs,
            origin: counter,
            last: counter,
            repeated: 0,
        })
    }
    pub fn observe(&mut self, counter: u64) -> Result<u64, TimeError> {
        if counter < self.last {
            return Err(TimeError::Backwards);
        }
        if counter == self.last {
            self.repeated += 1;
            if self.repeated >= STALLED_READS {
                return Err(TimeError::Stalled);
            }
        } else {
            self.repeated = 0;
        }
        self.last = counter;
        let ns = u128::from(counter - self.origin) * u128::from(self.period_fs) / 1_000_000;
        ns.try_into().map_err(|_| TimeError::Overflow)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitError {
    Deadline,
    PollBudgetExhausted,
    Clock(TimeError),
}
impl WaitError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Deadline => "DEADLINE",
            Self::PollBudgetExhausted => "POLL_BUDGET_EXHAUSTED",
            Self::Clock(e) => e.code(),
        }
    }
}
pub struct Wait {
    start: u64,
    deadline: u64,
    remaining: u64,
    last: u64,
}
impl Wait {
    pub fn new(start: u64, duration: u64, polls: u64) -> Result<Self, TimeError> {
        Ok(Self {
            start,
            deadline: start.checked_add(duration).ok_or(TimeError::Overflow)?,
            remaining: polls,
            last: start,
        })
    }
    pub fn poll(&mut self, now: Result<u64, TimeError>) -> Result<(), WaitError> {
        let now = now.map_err(WaitError::Clock)?;
        if now < self.last {
            return Err(WaitError::Clock(TimeError::Backwards));
        }
        self.last = now;
        if now >= self.deadline {
            return Err(WaitError::Deadline);
        }
        if self.remaining == 0 {
            return Err(WaitError::PollBudgetExhausted);
        }
        self.remaining -= 1;
        Ok(())
    }
    pub fn elapsed(&self) -> u64 {
        self.last - self.start
    }
}

pub type Queue<T> = BoundedQueue<T, QUEUE_CAPACITY>;
pub struct BoundedQueue<T: Copy, const N: usize> {
    entries: [Option<T>; N],
    head: usize,
    length: usize,
    rejected: u64,
    saturated: bool,
}
impl<T: Copy, const N: usize> Default for BoundedQueue<T, N> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T: Copy, const N: usize> BoundedQueue<T, N> {
    pub const fn new() -> Self {
        assert!(N > 0);
        Self {
            entries: [None; N],
            head: 0,
            length: 0,
            rejected: 0,
            saturated: false,
        }
    }
    pub fn push(&mut self, item: T) -> Result<(), T> {
        if self.length == N {
            if let Some(n) = self.rejected.checked_add(1) {
                self.rejected = n;
            } else {
                self.saturated = true;
            }
            return Err(item);
        }
        self.entries[(self.head + self.length) % N] = Some(item);
        self.length += 1;
        Ok(())
    }
    pub fn pop(&mut self) -> Option<T> {
        if self.length == 0 {
            return None;
        }
        let item = self.entries[self.head].take();
        self.head = (self.head + 1) % N;
        self.length -= 1;
        item
    }
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }
    pub fn clear(&mut self) {
        while self.pop().is_some() {}
    }
    pub fn rejected(&self) -> (u64, bool) {
        (self.rejected, self.saturated)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryScope {
    UsableFrames,
    Heap,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryStats {
    pub total_bytes: u64,
    pub tracked_bytes: u64,
    pub allocated_bytes: u64,
    pub scope: MemoryScope,
}
impl MemoryStats {
    pub fn new(total: u64, tracked: u64, allocated: u64, scope: MemoryScope) -> Option<Self> {
        (allocated <= tracked && tracked <= total).then_some(Self {
            total_bytes: total,
            tracked_bytes: tracked,
            allocated_bytes: allocated,
            scope,
        })
    }
    pub fn tracked_free(self) -> u64 {
        self.tracked_bytes - self.allocated_bytes
    }
    pub fn untracked(self) -> u64 {
        self.total_bytes - self.tracked_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clock_handles_resolution_stall_wrap_and_overflow() {
        assert!(matches!(Clock::new(0, 0), Err(TimeError::Period)));
        assert!(matches!(Clock::new(100_000_001, 0), Err(TimeError::Period)));
        let mut c = Clock::new(10_000_000, 0xffff_ffff).unwrap();
        assert_eq!(c.observe(0x1_0000_0001), Ok(20));
        assert_eq!(c.observe(0x1_0000_0001), Ok(20));
        assert_eq!(c.observe(0), Err(TimeError::Backwards));
        let mut c = Clock::new(1, 5).unwrap();
        for _ in 1..STALLED_READS {
            assert_eq!(c.observe(5), Ok(0));
        }
        assert_eq!(c.observe(5), Err(TimeError::Stalled));
        assert_eq!(
            Clock::new(100_000_000, 0).unwrap().observe(u64::MAX),
            Err(TimeError::Overflow)
        );
    }
    #[test]
    fn wait_distinguishes_deadline_budget_and_clock_error() {
        let mut w = Wait::new(10, 5, 2).unwrap();
        assert_eq!(w.poll(Ok(10)), Ok(()));
        assert_eq!(w.poll(Ok(14)), Ok(()));
        assert_eq!(w.poll(Ok(14)), Err(WaitError::PollBudgetExhausted));
        assert_eq!(w.poll(Ok(15)), Err(WaitError::Deadline));
        assert_eq!(w.elapsed(), 5);
        assert_eq!(
            w.poll(Err(TimeError::Stalled)),
            Err(WaitError::Clock(TimeError::Stalled))
        );
        assert!(Wait::new(u64::MAX, 1, 2).is_err());
        assert_eq!(
            Wait::new(5, 8, 1).unwrap().poll(Ok(4)),
            Err(WaitError::Clock(TimeError::Backwards))
        );
    }
    #[test]
    fn queue_preserves_order_wraps_and_reports_overflow() {
        let mut q = Queue::new();
        for i in 0..64 {
            assert_eq!(q.push(i), Ok(()));
        }
        assert_eq!(q.push(64), Err(64));
        assert_eq!(q.rejected(), (1, false));
        for i in 0..32 {
            assert_eq!(q.pop(), Some(i));
        }
        for i in 64..96 {
            q.push(i).unwrap();
        }
        for i in 32..96 {
            assert_eq!(q.pop(), Some(i));
        }
        assert_eq!(q.pop(), None);
        assert!(q.is_empty());
        for i in 0..64 {
            q.push(i).unwrap();
        }
        q.rejected = u64::MAX;
        assert_eq!(q.push(99), Err(99));
        assert_eq!(q.rejected(), (u64::MAX, true));
    }
    #[test]
    fn memory_unknown_is_never_free() {
        let m = MemoryStats::new(10000, 6000, 2000, MemoryScope::UsableFrames).unwrap();
        assert_eq!(m.tracked_free(), 4000);
        assert_eq!(m.untracked(), 4000);
        assert!(MemoryStats::new(1, 2, 0, MemoryScope::Heap).is_none());
        assert!(MemoryStats::new(2, 2, 3, MemoryScope::Heap).is_none());
    }
}
