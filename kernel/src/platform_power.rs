//! 仅固定 q35 平台；同步 / DMA 停止成功后才允许电源端口请求。
use crate::{boot::Facts, clock, file_runtime::Service, platform_memory, serial::Serial};
use core::fmt::Write;
use radishroot_kernel::{
    acpi,
    services::{
        Cause, PlatformCause, Stage,
        power::{Action, State},
    },
};
use x86_64::instructions::port::Port;
pub struct Controller {
    registers: Result<acpi::power::Registers, PlatformCause>,
    prepared_since: Option<u64>,
    issued_since: Option<u64>,
}
impl Controller {
    pub fn new(facts: &Facts) -> Self {
        let registers = clock::acpi_table(facts, b"FACP")
            .map_err(PlatformCause::Discovery)
            .and_then(|b| acpi::power::q35(b).map_err(PlatformCause::Acpi));
        #[cfg(feature = "boot-test")]
        let registers = if facts.mode == "t12-platform-fail" {
            Err(PlatformCause::Acpi(acpi::Error::Unsupported))
        } else {
            registers
        };
        Self {
            registers,
            prepared_since: None,
            issued_since: None,
        }
    }
    pub fn turn(&mut self, serial: &mut Serial, files: &mut Service<'_>, now: u64) {
        if let Some((action, id)) = files.prepared_power() {
            let since = *self.prepared_since.get_or_insert_with(|| {
                let _ = writeln!(
                    serial,
                    "RR:T12:PREPARED action={action:?} operation={} sync_confirmed=1",
                    id.sequence
                );
                now
            });
            // 先让共用工作台绘出 Prepared，硬件按键释放也能在下一轮消费。
            if now.saturating_sub(since) < 200_000_000 {
                return;
            }
            let registers = match self.registers {
                Ok(r) => r,
                Err(e) => {
                    files.fail_power(Stage::Platform, Cause::Platform(e));
                    let _ = writeln!(
                        serial,
                        "RR:T12:BLOCKED stage=Platform cause={e:?} request_sent=0"
                    );
                    return;
                }
            };
            if files.quiesce_for_power().is_err() {
                let _ = writeln!(serial, "RR:T12:BLOCKED stage=DeviceStop request_sent=0");
                return;
            }
            let (allocations, shared) = platform_memory::allocations();
            let _ = writeln!(
                serial,
                "RR:T12:STOP reset=confirmed bus_master=0 allocations={allocations} shared={shared}"
            );
            files.power_requested();
            let (port, value) = match action {
                Action::Reboot => (registers.reset_port, u16::from(registers.reset_value)),
                Action::Shutdown => (registers.pm1_control, 1 << 13),
                Action::Sync => unreachable!("sync never prepares power"),
            };
            let _ = writeln!(
                serial,
                "RR:T12:REQUEST action={action:?} operation={} port={port:#x} value={value:#x}",
                id.sequence
            );
            self.issued_since = Some(now);
            x86_64::instructions::interrupts::without_interrupts(|| {
                // SAFETY: CPL0、唯一 BSP；FADT / 固定 q35 端口和访问宽度已验证，文件同步与设备停止均成功。
                // 无 DMA 借用或后台服务；只执行所选动作一次，返回后按时限判失败，不 halt 冒充成功。
                unsafe {
                    match action {
                        Action::Reboot => Port::<u8>::new(port).write(value as u8),
                        Action::Shutdown => {
                            let mut p = Port::<u16>::new(port);
                            let old = p.read();
                            p.write((old & !((7 << 10) | (1 << 13))) | value);
                        }
                        Action::Sync => unreachable!(),
                    }
                }
            });
        } else if matches!(files.power(), State::Requested(..))
            && self
                .issued_since
                .is_some_and(|s| now.saturating_sub(s) > 500_000_000)
        {
            files.fail_power(Stage::Platform, Cause::Platform(PlatformCause::NotTaken));
            let _ = writeln!(
                serial,
                "RR:T12:BLOCKED stage=Platform code=PowerNotTaken request_sent=1"
            );
        }
    }
}
