//! COM1 为最小终端入口；图形 UI 后续复用 shell / editor，不复制文件操作。
use crate::serial::Serial;
use core::fmt::Write;
use radishroot_kernel::{
    services::{Backend, FileService},
    shell::{
        Shell,
        input::{Action, Input},
        render,
    },
};
pub struct Console {
    shell: Shell,
    input: Input,
    received: u64,
    pub completed: u64,
    escape_since: Option<u64>,
}
impl Console {
    pub fn new() -> Self {
        Self {
            shell: Shell::new(),
            input: Input::new(),
            received: 0,
            completed: 0,
            escape_since: None,
        }
    }
    pub fn announce<B: Backend>(
        &self,
        serial: &mut Serial,
        files: &FileService<B>,
    ) -> Result<(), &'static str> {
        writeln!(
            serial,
            "RR:CONSOLE:READY boot_id={:032x} line_bytes=1024 history=16 cwd=/data",
            u128::from_be_bytes(files.boot_id().0)
        )
        .map_err(|_| "SERIAL")
    }
    fn action<B: Backend>(
        &mut self,
        action: Action,
        serial: &mut Serial,
        files: &mut FileService<B>,
    ) -> Result<(), &'static str> {
        match action {
            Action::None => (),
            Action::Changed => {
                write!(
                    serial,
                    "RR:CONSOLE:EDIT {{\"seq\":{},\"cursor\":{},\"history_evicted\":{},\"line\":",
                    self.received,
                    self.input.editor.cursor(),
                    self.input.editor.evicted
                )
                .map_err(|_| "SERIAL")?;
                render::string(serial, self.input.editor.text()).map_err(|_| "SERIAL")?;
                writeln!(serial, "}}").map_err(|_| "SERIAL")?;
            }
            Action::Rejected(error) => {
                writeln!(
                    serial,
                    "RR:CONSOLE:INPUT_ERROR code={error:?} line_discarded=1"
                )
                .map_err(|_| "SERIAL")?;
            }
            Action::Submitted(line) => {
                let reply = self
                    .shell
                    .execute(&line, files)
                    .map_err(|_| "COMMAND_IDENTITY")?;
                write!(serial, "RR:CONSOLE:REPLY ").map_err(|_| "SERIAL")?;
                render::reply(serial, &reply).map_err(|_| "SERIAL")?;
                writeln!(serial).map_err(|_| "SERIAL")?;
                self.completed = self.completed.checked_add(1).ok_or("COMMAND_COUNT")?;
            }
        }
        Ok(())
    }
    pub fn turn<B: Backend>(
        &mut self,
        serial: &mut Serial,
        files: &mut FileService<B>,
        now: u64,
    ) -> Result<bool, &'static str> {
        if self
            .escape_since
            .is_some_and(|start| now.saturating_sub(start) >= 100_000_000)
        {
            let action = self.input.escape_timeout();
            self.escape_since = None;
            self.action(action, serial, files)?;
        }
        for _ in 0..32 {
            let byte = match serial.receive() {
                Ok(Some(byte)) => byte,
                Ok(None) => return Ok(false),
                Err(status) => {
                    let action = self.input.transport_error();
                    writeln!(serial, "RR:CONSOLE:TRANSPORT status={status:#x}")
                        .map_err(|_| "SERIAL")?;
                    self.action(action, serial, files)?;
                    return Ok(false);
                }
            };
            self.received = self.received.checked_add(1).ok_or("INPUT_SEQUENCE")?;
            let action = self.input.receive(byte);
            self.action(action, serial, files)?;
            if self.input.pending_escape() {
                self.escape_since.get_or_insert(now);
            } else {
                self.escape_since = None;
            }
            writeln!(serial, "RR:CONSOLE:INPUT seq={}", self.received).map_err(|_| "SERIAL")?;
        }
        Ok(true)
    }
}
