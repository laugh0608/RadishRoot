use core::fmt;
use uart_16550::{Config, Uart16550, backend::PioBackend};

pub struct Serial(Uart16550<PioBackend>);
impl Serial {
    pub fn new() -> Result<Self, ()> {
        // SAFETY: 固定 q35 的 COM1 端口 0x3f8..0x3ff；单 BSP，IRQ 从不访问串口。
        // 主流程的诊断实例顺序访问同一设备；致命路径禁中断且永不返回，接管暂停的上下文。
        let mut uart = unsafe { Uart16550::new_port(0x3f8) }.map_err(|_| ())?;
        uart.init(Config::default()).map_err(|_| ())?;
        Ok(Self(uart))
    }
    /// 单 BSP 非阻塞接收；LSR 保留发送过程中观测到的接收错误，不能静默吃掉。
    pub fn receive(&mut self) -> Result<Option<u8>, u8> {
        let before = self.0.lsr().bits() & 0x1e;
        let byte = self.0.try_receive_byte().ok();
        let errors = before | (self.0.lsr().bits() & 0x1e);
        if errors != 0 { Err(errors) } else { Ok(byte) }
    }
    fn byte(&mut self, byte: u8) -> fmt::Result {
        for _ in 0..100_000 {
            if self.0.try_send_byte(byte).is_ok() {
                return Ok(());
            }
            core::hint::spin_loop();
        }
        Err(fmt::Error)
    }
}
impl fmt::Write for Serial {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for byte in text.bytes() {
            if byte == b'\n' {
                self.byte(b'\r')?;
            }
            self.byte(byte)?;
        }
        Ok(())
    }
}
