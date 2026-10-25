//! 固定 modern VirtIO-blk 后端：库提供 PCI / queue，项目拥有有界协议与资源终态。
use crate::{
    pci,
    platform_memory::{self, VirtioHal},
    serial::Serial,
};
use core::{fmt::Write, mem::ManuallyDrop};
use radishroot_kernel::runtime_contract::{
    REQUEST_NS, REQUEST_POLLS, RESET_NS, RESET_POLLS, Wait, WaitError,
};
use radishroot_kernel::storage::block::{self, BlockDevice, Error, Info, Kind, Stage};
use virtio_drivers::{
    queue::VirtQueue,
    transport::{
        DeviceStatus, Transport,
        pci::{PciTransport, bus::DeviceFunction},
    },
};

const VERSION_1: u64 = 1 << 32;
const RO: u64 = 1 << 5;
const FLUSH: u64 = 1 << 9;
const QUEUE_SIZE: usize = 16;

pub struct Driver {
    transport: ManuallyDrop<PciTransport>,
    queue: ManuallyDrop<VirtQueue<VirtioHal, QUEUE_SIZE>>,
    bdf: DeviceFunction,
    info: Info,
    stopped: bool,
    timeout_next: bool,
    deadline_test: bool,
    unsupported_next: bool,
    pub requests: u64,
    pub flushes: u64,
}
fn protocol(stage: Stage) -> Error {
    Error::new(Kind::Protocol, stage, 0)
}
impl Driver {
    pub fn new(device: pci::Device, reject_flush: bool) -> Result<Self, Error> {
        let mut transport = device.transport;
        let features = transport.read_device_features();
        if features & VERSION_1 == 0 {
            return Err(Error::new(Kind::Configuration, Stage::Initialize, 0));
        }
        if features & FLUSH == 0 || reject_flush {
            return Err(Error::new(Kind::FlushUnsupported, Stage::Initialize, 0));
        }
        // 不调用 begin_init / read_consistent：这些库 helper 的等待或重试没有预算。
        transport.set_status(DeviceStatus::ACKNOWLEDGE | DeviceStatus::DRIVER);
        transport.write_driver_features(features & (VERSION_1 | RO | FLUSH));
        let negotiated =
            DeviceStatus::ACKNOWLEDGE | DeviceStatus::DRIVER | DeviceStatus::FEATURES_OK;
        transport.set_status(negotiated);
        if transport.get_status() != negotiated {
            return Err(protocol(Stage::Initialize));
        }
        let mut sectors = None;
        for _ in 0..64 {
            let before = transport.read_config_generation();
            let low: u32 = transport
                .read_config_space(0)
                .map_err(|_| protocol(Stage::Initialize))?;
            let high: u32 = transport
                .read_config_space(4)
                .map_err(|_| protocol(Stage::Initialize))?;
            if transport.read_config_generation() == before {
                sectors = Some(u64::from(low) | (u64::from(high) << 32));
                break;
            }
        }
        let sectors = sectors.ok_or(protocol(Stage::Initialize))?;
        if !(68..=u64::MAX / 512).contains(&sectors) {
            return Err(Error::new(Kind::Range, Stage::Initialize, 0));
        }
        let mut queue = ManuallyDrop::new(
            VirtQueue::new(&mut *transport, 0, false, false)
                .map_err(|_| protocol(Stage::Initialize))?,
        );
        queue.set_dev_notify(false);
        platform_memory::device_enabled().map_err(|_| protocol(Stage::Initialize))?;
        pci::bus_master(device.bdf, true).map_err(|_| protocol(Stage::Initialize))?;
        transport.set_status(negotiated | DeviceStatus::DRIVER_OK);
        if transport.get_status() != negotiated | DeviceStatus::DRIVER_OK {
            return Err(protocol(Stage::Initialize));
        }
        Ok(Self {
            transport,
            queue,
            bdf: device.bdf,
            info: Info {
                sectors,
                readonly: features & RO != 0,
                flush: true,
            },
            stopped: false,
            timeout_next: false,
            deadline_test: false,
            unsupported_next: false,
            requests: 0,
            flushes: 0,
        })
    }
    pub fn inject_timeout(&mut self) {
        self.timeout_next = true;
    }
    pub fn inject_deadline_timeout(&mut self) {
        if !cfg!(feature = "boot-test") {
            panic!("fault injection requires boot-test");
        }
        self.timeout_next = true;
        self.deadline_test = true;
    }
    pub fn inject_unsupported(&mut self) {
        self.unsupported_next = true;
    }
    fn header(&mut self, operation: u32, lba: u64) -> [u8; 16] {
        let mut header = [0u8; 16];
        let kind = if self.unsupported_next {
            self.unsupported_next = false;
            u32::MAX
        } else {
            operation
        };
        header[..4].copy_from_slice(&kind.to_le_bytes());
        header[8..].copy_from_slice(&lba.to_le_bytes());
        header
    }
    fn request<'a>(
        &mut self,
        inputs: &'a [&'a [u8]],
        outputs: &'a mut [&'a mut [u8]],
        stage: Stage,
        lba: u64,
    ) -> Result<u32, Error> {
        if self.stopped {
            return Err(Error::new(Kind::Offline, stage, lba));
        }
        let start = crate::clock::now().map_err(|_| Error::new(Kind::Clock, stage, lba))?;
        let mut wait = Wait::new(
            start,
            REQUEST_NS,
            if self.deadline_test {
                u64::MAX
            } else {
                REQUEST_POLLS
            },
        )
        .map_err(|_| Error::new(Kind::Clock, stage, lba))?;
        platform_memory::begin_request().map_err(|_| protocol(stage))?;
        // SAFETY: header / data / status 保持在本次调用栈，唯一请求；所有地址由 HAL bounce 管理。
        // 正常路径直到匹配 used token 才 pop / copy-back；异常未完成路径在本栈帧内 halt，绝不返回。
        let added = unsafe { self.queue.add(inputs, outputs) };
        let token = match added {
            Ok(token) => token,
            Err(_) => {
                if platform_memory::cancel_empty_request().is_err() {
                    self.quarantine(protocol(stage));
                }
                return Err(protocol(stage));
            }
        };
        if platform_memory::publish_request().is_err() {
            self.quarantine(protocol(stage));
        }
        self.requests += 1;
        self.transport.notify(0);
        loop {
            if let Err(reason) = wait.poll(crate::clock::now()) {
                crate::runtime::wait_error(reason, wait.elapsed(), "request");
                self.quarantine(Error::new(wait_kind(reason), stage, lba));
            }
            // 注入只隐藏完成观察；真实请求已经发布 / notify，设备可能仍会写 DMA。
            if !self.timeout_next {
                if let Some(used) = self.queue.peek_used() {
                    if used != token {
                        self.quarantine(Error::new(Kind::Protocol, Stage::Complete, lba));
                    }
                    break;
                }
                if self
                    .transport
                    .get_status()
                    .intersects(DeviceStatus::DEVICE_NEEDS_RESET | DeviceStatus::FAILED)
                {
                    self.quarantine(Error::new(Kind::Protocol, stage, lba));
                }
            }
            core::hint::spin_loop();
        }
        if platform_memory::complete_request().is_err() {
            self.quarantine(protocol(Stage::Complete));
        }
        // SAFETY: acquire used.idx 已表明唯一请求完成，token 匹配；原始 slice 与 add 完全相同。
        let length = unsafe { self.queue.pop_used(token, inputs, outputs) };
        let length = match length {
            Ok(length) => length,
            Err(_) => self.quarantine(protocol(Stage::Complete)),
        };
        platform_memory::finish_request().map_err(|_| protocol(Stage::Complete))?;
        Ok(length)
    }
    fn response(
        status: u8,
        used: u32,
        expected: usize,
        stage: Stage,
        lba: u64,
    ) -> Result<(), Error> {
        let kind = match status {
            0 if used as usize == expected => return Ok(()),
            0 => Kind::Protocol,
            1 => Kind::Io,
            2 => Kind::Unsupported,
            _ => Kind::Protocol,
        };
        Err(Error::new(kind, stage, lba))
    }
    fn quarantine(&mut self, error: Error) -> ! {
        self.stopped = true;
        // 不返回到借用方，也不执行 queue / transport Drop。故障不意味着 DMA 已停止。
        // 原缓冲、bounce、queue 保留；设备不再接新请求，整个 guest 随后受控 halt。
        if let Ok(mut serial) = Serial::new() {
            match platform_memory::quarantine_request() {
                Ok(q) => {
                    let _ = writeln!(
                        serial,
                        "RR:T04:QUARANTINE shares={} allocations={} release_rejected={} allocation_rejected={} reused=0",
                        q.shares,
                        q.allocations,
                        u8::from(q.release_rejected),
                        u8::from(q.allocation_rejected)
                    );
                }
                Err(reason) => {
                    let _ = writeln!(serial, "RR:T04:QUARANTINE_ERROR reason={reason}");
                }
            }
            let _ = writeln!(
                serial,
                "RR:T04:FAIL code={} stage={:?} sector={} completed_bytes={} partial_effect=unknown",
                error.kind.code(),
                error.stage,
                error.sector,
                error.completed_bytes
            );
            if self.deadline_test {
                let _ = writeln!(
                    serial,
                    "RR:T06:FAIL code={} stage=request",
                    error.kind.code()
                );
            }
        }
        crate::halt()
    }
    pub fn stop(&mut self) -> Result<(), Error> {
        if self.stopped {
            return Err(Error::new(Kind::Offline, Stage::Reset, 0));
        }
        let start = crate::clock::now().map_err(|_| Error::new(Kind::Clock, Stage::Reset, 0))?;
        let mut wait = Wait::new(start, RESET_NS, RESET_POLLS)
            .map_err(|_| Error::new(Kind::Clock, Stage::Reset, 0))?;
        self.transport.set_status(DeviceStatus::empty());
        loop {
            if let Err(reason) = wait.poll(crate::clock::now()) {
                crate::runtime::wait_error(reason, wait.elapsed(), "reset");
                // 无未完成借用请求，但 queue 仍可能被访问；ManuallyDrop 保留到调用方立即停止。
                self.stopped = true;
                return Err(Error::new(wait_kind(reason), Stage::Reset, 0));
            }
            if self.transport.get_status().is_empty() {
                pci::bus_master(self.bdf, false).map_err(|_| protocol(Stage::Reset))?;
                platform_memory::device_stopped().map_err(|_| protocol(Stage::Reset))?;
                // SAFETY: 真实 status=0 已确认复位完成，PCI bus master 已关闭，无 outstanding request。
                // 此处唯一释放 queue；PciTransport 继续保留，避免其无界 Drop 和重复 MMIO claim。
                unsafe {
                    ManuallyDrop::drop(&mut self.queue);
                }
                self.stopped = true;
                if platform_memory::allocations() != (0, 0) {
                    return Err(protocol(Stage::Reset));
                }
                return Ok(());
            }
            core::hint::spin_loop();
        }
    }
}
fn wait_kind(reason: WaitError) -> Kind {
    match reason {
        WaitError::Deadline => Kind::Timeout,
        WaitError::PollBudgetExhausted => Kind::PollBudgetExhausted,
        WaitError::Clock(_) => Kind::Clock,
    }
}
impl BlockDevice for Driver {
    fn info(&self) -> Info {
        self.info
    }
    fn read_blocks(&mut self, lba: u64, bytes: &mut [u8]) -> Result<(), Error> {
        block::validate(self.info, lba, bytes.len(), false)?;
        let header = self.header(0, lba);
        let mut status = [0xffu8];
        let expected = bytes.len() + 1;
        let used = self.request(&[&header], &mut [bytes, &mut status], Stage::Read, lba)?;
        Self::response(status[0], used, expected, Stage::Read, lba)
    }
    fn write_blocks(&mut self, lba: u64, bytes: &[u8]) -> Result<(), Error> {
        block::validate(self.info, lba, bytes.len(), true)?;
        let header = self.header(1, lba);
        let mut status = [0xffu8];
        let used = self.request(&[&header, bytes], &mut [&mut status], Stage::Write, lba)?;
        Self::response(status[0], used, 1, Stage::Write, lba)
    }
    fn flush(&mut self) -> Result<(), Error> {
        if !self.info.flush {
            return Err(Error::new(Kind::FlushUnsupported, Stage::Flush, 0));
        }
        let header = self.header(4, 0);
        let mut status = [0xffu8];
        let used = self.request(&[&header], &mut [&mut status], Stage::Flush, 0)?;
        Self::response(status[0], used, 1, Stage::Flush, 0)?;
        self.flushes += 1;
        Ok(())
    }
}
