//! 固定 q35 i8042：初始化事务有界，ISR 只采集，主循环消费并负责恢复。
use crate::{clock, interrupts};
use radishroot_kernel::{
    input::{self, Port as InputPort, Raw},
    runtime_contract::Wait,
};
use x86_64::instructions::{interrupts as cpu, port::Port};

#[derive(Clone, Copy, Debug)]
pub struct Error {
    pub code: &'static str,
    pub stage: &'static str,
    pub port: InputPort,
}
static mut RAW: input::RawQueue = input::RawQueue::new();
fn read(port: u16) -> u8 {
    // SAFETY: 固定 i8042 状态/数据 PIO，唯一 BSP；调用处通过 IF=0 或屏蔽输入 IRQ 保证互斥。
    unsafe { Port::<u8>::new(port).read() }
}
fn write(port: u16, value: u8) {
    // SAFETY: 仅固定 i8042 命令/数据端口，不写控制 A20/复位的输出端口命令。
    unsafe {
        Port::<u8>::new(port).write(value);
    }
}
fn receive() -> Option<Raw> {
    let status = read(0x64);
    (status & 1 != 0).then(|| Raw {
        port: if status & 0x20 != 0 {
            InputPort::Mouse
        } else {
            InputPort::Keyboard
        },
        byte: read(0x60),
        errors: status & 0xc0,
    })
}
fn capture() {
    // SAFETY: ISR IF=0 或主循环 without_interrupts，单 BSP；引用只活在本次短采集内。
    let state = unsafe { &mut *core::ptr::addr_of_mut!(RAW) };
    for _ in 0..input::RAW_PER_IRQ {
        let Some(raw) = receive() else {
            break;
        };
        state.push(raw);
    }
}
pub fn irq(slave: bool) {
    capture();
    // SAFETY: IRQ1 来自主 PIC；IRQ12 先应答 slave 后应答 master，固定路由。
    unsafe {
        if slave {
            Port::<u8>::new(0xa0).write(0x20);
        }
        Port::<u8>::new(0x20).write(0x20);
    }
    interrupts::wake();
}
pub fn drain(paused: bool) -> input::RawBatch {
    cpu::without_interrupts(|| {
        capture();
        // SAFETY: capture 引用已结束，单 BSP / IF=0；批量值复制出去，不借出队列。
        unsafe { (&mut *core::ptr::addr_of_mut!(RAW)).drain(paused) }
    })
}
pub fn clear_after_reset() {
    cpu::without_interrupts(|| {
        // SAFETY: 唯一 BSP / IF=0，输入 IRQ 屏蔽；引用不逃离临界区。
        unsafe {
            (&mut *core::ptr::addr_of_mut!(RAW)).clear_after_reset();
        }
    });
}
struct Controller {
    total: Wait,
    stage: &'static str,
    port: InputPort,
    hide_ack: bool,
}
impl Controller {
    fn error(&self, code: &'static str) -> Error {
        Error {
            code,
            stage: self.stage,
            port: self.port,
        }
    }
    fn wait(&self, ns: u64) -> Result<Wait, Error> {
        Wait::new(
            clock::now().map_err(|e| self.error(e.code()))?,
            ns,
            input::STEP_POLLS,
        )
        .map_err(|e| self.error(e.code()))
    }
    fn poll(&mut self, step: &mut Wait) -> Result<(), Error> {
        let now = clock::now();
        self.total.poll(now).map_err(|e| self.error(e.code()))?;
        step.poll(now).map_err(|e| self.error(e.code()))
    }
    fn send(&mut self, port: u16, byte: u8) -> Result<(), Error> {
        let mut wait = self.wait(input::COMMAND_NS)?;
        loop {
            self.poll(&mut wait)?;
            if read(0x64) & 2 == 0 {
                write(port, byte);
                return Ok(());
            }
            core::hint::spin_loop();
        }
    }
    fn reply(&mut self, port: InputPort, ns: u64) -> Result<u8, Error> {
        let mut wait = self.wait(ns)?;
        let mut ack_hidden = false;
        loop {
            self.poll(&mut wait)?;
            if let Some(raw) = receive() {
                let byte = input::transaction_byte(raw, port).map_err(|e| self.error(e))?;
                // 隐藏 reset ACK 后，BAT 属于同一失去应答的事务，不能越过 ACK 被当作新回复。
                // 继续读取真实设备但隔离该事务后续字节，直到原等待预算明确超时。
                if self.hide_ack && self.stage == "keyboard-reset" && byte == 0xfa {
                    ack_hidden = true;
                }
                if ack_hidden {
                    continue;
                }
                return Ok(byte);
            }
            core::hint::spin_loop();
        }
    }
    fn command(&mut self, command: u8) -> Result<(), Error> {
        self.send(0x64, command)
    }
    fn configuration(&mut self) -> Result<u8, Error> {
        self.command(0x20)?;
        self.reply(InputPort::Keyboard, input::COMMAND_NS)
    }
    fn set_configuration(&mut self, value: u8) -> Result<(), Error> {
        self.command(0x60)?;
        self.send(0x60, value)?;
        if self.configuration()? != value {
            return Err(self.error("PS2_CONFIG_READBACK"));
        }
        Ok(())
    }
    fn device(&mut self, port: InputPort, byte: u8) -> Result<(), Error> {
        self.port = port;
        for retry in 0..=input::RESENDS {
            if port == InputPort::Mouse {
                self.command(0xd4)?;
            }
            self.send(0x60, byte)?;
            match self.reply(port, input::COMMAND_NS)? {
                0xfa => return Ok(()),
                0xfe if retry < input::RESENDS => continue,
                0xfe => return Err(self.error("PS2_RESEND_EXHAUSTED")),
                _ => return Err(self.error("PS2_UNEXPECTED_REPLY")),
            }
        }
        unreachable!()
    }
    fn expected(&mut self, byte: u8, ns: u64) -> Result<(), Error> {
        if self.reply(self.port, ns)? != byte {
            return Err(self.error("PS2_UNEXPECTED_REPLY"));
        }
        Ok(())
    }
    fn initialize(&mut self) -> Result<(), Error> {
        self.command(0xad)?;
        self.command(0xa7)?;
        let mut drained = 0;
        while receive().is_some() {
            drained += 1;
            if drained >= 256 {
                return Err(self.error("PS2_DRAIN_LIMIT"));
            }
        }
        let config = self.configuration()? & !0x43;
        self.set_configuration(config | 0x30)?;
        self.stage = "controller-self-test";
        self.command(0xaa)?;
        self.expected(0x55, input::COMMAND_NS)?;
        self.set_configuration(config | 0x30)?;
        self.stage = "keyboard-port-test";
        self.command(0xab)?;
        self.expected(0, input::COMMAND_NS)?;
        self.stage = "mouse-port-test";
        self.command(0xa9)?;
        self.expected(0, input::COMMAND_NS)?;
        self.command(0xae)?;
        self.command(0xa8)?;
        self.set_configuration(config & !0x30)?;
        self.stage = "keyboard-reset";
        self.device(InputPort::Keyboard, 0xff)?;
        self.expected(0xaa, input::DEVICE_RESET_NS)?;
        self.device(InputPort::Keyboard, 0xf5)?;
        self.stage = "keyboard-set2";
        self.device(InputPort::Keyboard, 0xf0)?;
        self.device(InputPort::Keyboard, 2)?;
        self.device(InputPort::Keyboard, 0xf0)?;
        self.device(InputPort::Keyboard, 0)?;
        self.expected(2, input::COMMAND_NS)?;
        self.stage = "mouse-reset";
        self.device(InputPort::Mouse, 0xff)?;
        self.expected(0xaa, input::DEVICE_RESET_NS)?;
        self.expected(0, input::COMMAND_NS)?;
        self.device(InputPort::Mouse, 0xf5)?;
        self.stage = "mouse-stream";
        self.device(InputPort::Mouse, 0xe6)?;
        self.device(InputPort::Mouse, 0xea)?;
        // 两设备配置完成后才启用；初始化期间外部输入的非事务回复明确拒绝，不默默当 ACK。
        self.stage = "mouse-enable";
        self.device(InputPort::Mouse, 0xf4)?;
        self.stage = "keyboard-enable";
        self.device(InputPort::Keyboard, 0xf4)?;
        self.stage = "irq-enable";
        self.set_configuration((config & !0x30) | 3)?;
        Ok(())
    }
}
pub fn initialize(hide_ack: bool) -> Result<(), Error> {
    interrupts::input_enabled(false);
    let start = clock::now().map_err(|e| Error {
        code: e.code(),
        stage: "clock",
        port: InputPort::Keyboard,
    })?;
    let total =
        Wait::new(start, input::INITIALIZE_NS, input::STEP_POLLS * 64).map_err(|e| Error {
            code: e.code(),
            stage: "clock",
            port: InputPort::Keyboard,
        })?;
    let result = Controller {
        total,
        stage: "disable",
        port: InputPort::Keyboard,
        hide_ack,
    }
    .initialize();
    if result.is_ok() {
        clear_after_reset();
        interrupts::input_enabled(true);
    }
    result
}
