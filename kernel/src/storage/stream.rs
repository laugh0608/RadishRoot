//! FAT 的唯一 DATA 字节流入口；不缓存写入，首次失败后禁止析构或重试再次访问设备。
use super::{BlockDevice, Partition, block};
use core::cell::Cell;
use fatfs::{IoBase, IoError, Read, Seek, SeekFrom, Write};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Mount,
    Read,
    Create,
    Write,
    FileSync,
    VolumeSync,
    Unmount,
    DeviceFlush,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    Block(block::Error),
    Bounds,
    UnexpectedEof,
    WriteZero,
    Budget,
    Aborted,
    InjectedWrite,
    InjectedFlush,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    pub phase: Phase,
    pub cause: Cause,
    pub offset: u64,
}
impl IoError for Error {
    fn is_interrupted(&self) -> bool {
        false
    }
    fn new_unexpected_eof_error() -> Self {
        Self {
            phase: Phase::Read,
            cause: Cause::UnexpectedEof,
            offset: 0,
        }
    }
    fn new_write_zero_error() -> Self {
        Self {
            phase: Phase::Write,
            cause: Cause::WriteZero,
            offset: 0,
        }
    }
}

/// 与挂载实例同寿命的首错及阶段记录。只供单线程同步 IO 使用，不是隐式全局状态。
pub struct State {
    phase: Cell<Phase>,
    first: Cell<Option<Error>>,
    calls: Cell<u32>,
    blocked: Cell<u32>,
    writes: Cell<u32>,
    #[cfg(any(test, feature = "boot-test"))]
    fail_flush: Cell<bool>,
    #[cfg(any(test, feature = "boot-test"))]
    fail_write: Cell<bool>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            phase: Cell::new(Phase::Mount),
            first: Cell::new(None),
            calls: Cell::new(0),
            blocked: Cell::new(0),
            writes: Cell::new(0),
            #[cfg(any(test, feature = "boot-test"))]
            fail_flush: Cell::new(false),
            #[cfg(any(test, feature = "boot-test"))]
            fail_write: Cell::new(false),
        }
    }
}
impl State {
    pub fn phase(&self, phase: Phase) {
        self.phase.set(phase);
    }
    pub fn first_error(&self) -> Option<Error> {
        self.first.get()
    }
    pub fn blocked_calls(&self) -> u32 {
        self.blocked.get()
    }
    pub fn writes(&self) -> u32 {
        self.writes.get()
    }
    #[cfg(any(test, feature = "boot-test"))]
    pub fn inject_flush_failure(&self) {
        self.fail_flush.set(true);
    }
    pub fn calls(&self) -> u32 {
        self.calls.get()
    }
    pub fn check(&self) -> Result<(), Error> {
        self.first.get().map_or(Ok(()), Err)
    }
    pub fn fail(&self, cause: Cause, offset: u64) -> Error {
        let error = self.first.get().unwrap_or(Error {
            phase: self.phase.get(),
            cause,
            offset,
        });
        self.first.set(Some(error));
        error
    }
    #[cfg(any(test, feature = "boot-test"))]
    pub fn inject_write_failure(&self) {
        self.fail_write.set(true);
    }
    fn enter(&self, offset: u64) -> Result<(), Error> {
        if let Some(error) = self.first.get() {
            self.blocked.set(self.blocked.get().saturating_add(1));
            return Err(error);
        }
        // 保护库遍历损坏簇链等异常路径；这是单挂载次数预算，不是时钟超时保证。
        if self.calls.get() == 100_000 {
            return Err(self.fail(Cause::Budget, offset));
        }
        self.calls.set(self.calls.get() + 1);
        Ok(())
    }
}

pub struct Stream<'a, D: BlockDevice> {
    partition: Partition<'a, D>,
    position: u64,
    state: &'a State,
}
impl<'a, D: BlockDevice> Stream<'a, D> {
    pub fn new(partition: Partition<'a, D>, state: &'a State) -> Self {
        Self {
            partition,
            position: 0,
            state,
        }
    }
    pub fn readonly(&self) -> bool {
        self.partition.readonly()
    }
    pub fn position(&self) -> u64 {
        self.position
    }
}
// 实现在借用上，使显式 unmount 消耗 FileSystem 后仍能执行最后一次设备 flush。
impl<D: BlockDevice> IoBase for &mut Stream<'_, D> {
    type Error = Error;
}
impl<D: BlockDevice> Seek for &mut Stream<'_, D> {
    fn seek(&mut self, from: SeekFrom) -> Result<u64, Error> {
        self.state.enter(self.position)?;
        let next = match from {
            SeekFrom::Start(n) => Some(n),
            SeekFrom::Current(n) => self.position.checked_add_signed(n),
            SeekFrom::End(n) => self.partition.byte_len().checked_add_signed(n),
        };
        let next = next
            .filter(|n| *n <= self.partition.byte_len())
            .ok_or_else(|| self.state.fail(Cause::Bounds, self.position))?;
        self.position = next;
        Ok(next)
    }
}
impl<D: BlockDevice> Read for &mut Stream<'_, D> {
    fn read(&mut self, output: &mut [u8]) -> Result<usize, Error> {
        self.state.enter(self.position)?;
        let count = output
            .len()
            .min((self.partition.byte_len() - self.position) as usize)
            .min(block::SECTOR - (self.position % block::SECTOR as u64) as usize);
        if count == 0 {
            return Ok(0);
        }
        self.partition
            .read_at(self.position, &mut output[..count])
            .map_err(|e| self.state.fail(Cause::Block(e), self.position))?;
        self.position += count as u64;
        Ok(count)
    }
}
impl<D: BlockDevice> Write for &mut Stream<'_, D> {
    fn write(&mut self, input: &[u8]) -> Result<usize, Error> {
        self.state.enter(self.position)?;
        if input.is_empty() {
            return Ok(0);
        }
        if input.len() as u64 > self.partition.byte_len() - self.position {
            return Err(self.state.fail(Cause::Bounds, self.position));
        }
        #[cfg(any(test, feature = "boot-test"))]
        if self.state.fail_write.replace(false) {
            return Err(self.state.fail(Cause::InjectedWrite, self.position));
        }
        let count = input
            .len()
            .min(block::SECTOR - (self.position % block::SECTOR as u64) as usize);
        self.state.writes.set(self.state.writes.get() + 1);
        self.partition
            .write_at(self.position, &input[..count])
            .map_err(|e| self.state.fail(Cause::Block(e), self.position))?;
        self.position += count as u64;
        Ok(count)
    }
    fn flush(&mut self) -> Result<(), Error> {
        self.state.enter(self.position)?;
        // 只读挂载从未提交写入；库的只读句柄 Drop 也调用 flush，无脏数据可提交。
        if self.partition.readonly() {
            return Ok(());
        }
        #[cfg(any(test, feature = "boot-test"))]
        if self.state.fail_flush.replace(false) {
            return Err(self.state.fail(Cause::InjectedFlush, self.position));
        }
        self.partition
            .flush()
            .map_err(|e| self.state.fail(Cause::Block(e), self.position))
    }
}

#[cfg(test)]
mod tests;
