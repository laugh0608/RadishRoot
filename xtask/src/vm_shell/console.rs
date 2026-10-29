use super::{script, validate};
use crate::error::{Error, Result};
use std::{
    fs,
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
pub struct Session {
    socket: PathBuf,
    serial: PathBuf,
    directory: PathBuf,
    mode: String,
    id: [u8; 16],
    stream: Option<UnixStream>,
    input: Vec<u8>,
    sent: usize,
    last_progress: Instant,
    done: Option<Instant>,
}
impl Session {
    pub fn new(
        socket: &Path,
        serial: &Path,
        directory: &Path,
        mode: &str,
        id: [u8; 16],
    ) -> Result<Self> {
        let input = script(mode, id)?;
        fs::write(directory.join("console-input.bin"), &input)?;
        Ok(Self {
            socket: socket.into(),
            serial: serial.into(),
            directory: directory.into(),
            mode: mode.into(),
            id,
            stream: None,
            input,
            sent: 0,
            last_progress: Instant::now(),
            done: None,
        })
    }
    pub fn poll(&mut self) -> Result<bool> {
        let bytes = fs::read(&self.serial)?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err(Error::new("shell-output", "over 2 MiB"));
        }
        let end = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
        let output = std::str::from_utf8(&bytes[..end])
            .map_err(|_| Error::new("shell-output", "invalid UTF-8 frame"))?;
        if output
            .lines()
            .any(|l| l.starts_with("RR:T") && l.contains(":FAIL "))
        {
            return Err(Error::new("shell-guest", "failure terminal"));
        }
        if self.stream.is_none() {
            if !output.contains("RR:CONSOLE:READY ") {
                return Ok(false);
            }
            let stream = UnixStream::connect(&self.socket)?;
            stream.set_nonblocking(true)?;
            self.stream = Some(stream);
            self.last_progress = Instant::now();
        }
        let stream = self.stream.as_mut().unwrap();
        let mut buffer = [0u8; 4096];
        // 及时消费字符后端输出，不能依赖日志而让 socket 回压阻塞 guest。
        for _ in 0..32 {
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(_) => (),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e.into()),
            }
        }
        if output
            .lines()
            .any(|l| l == format!("RR:T09:READY mode={}", self.mode))
        {
            if self.sent != self.input.len() {
                return Err(Error::new("shell-input", "guest ended before all input"));
            }
            if self.done.is_none() {
                validate(output, &self.mode, self.id)?;
                self.done = Some(Instant::now());
                for (name, bad) in [
                    (
                        "missing-reply",
                        output
                            .lines()
                            .filter(|l| !l.starts_with("RR:CONSOLE:REPLY "))
                            .collect::<Vec<_>>()
                            .join("\n"),
                    ),
                    (
                        "false-success",
                        output.replace("\"ok\":false", "\"ok\":true"),
                    ),
                    (
                        "missing-stop",
                        output
                            .lines()
                            .filter(|l| !l.starts_with("RR:T09:STOP "))
                            .collect::<Vec<_>>()
                            .join("\n"),
                    ),
                ] {
                    // reopen 只有成功结果；另以错误启动身份作负对照。
                    let bad = if name == "false-success" && self.mode == "t09-reopen" {
                        output.replace(
                            &format!("boot_id={:032x}", u128::from_be_bytes(self.id)),
                            "boot_id=0",
                        )
                    } else {
                        bad
                    };
                    if validate(&bad, &self.mode, self.id).is_ok() {
                        return Err(Error::new("shell-negative", name));
                    }
                    fs::write(self.directory.join(format!("negative-{name}.log")), bad)?;
                }
            }
            return Ok(self
                .done
                .is_some_and(|at| at.elapsed() >= Duration::from_millis(250)));
        }
        if self.last_progress.elapsed() > Duration::from_secs(5) {
            return Err(Error::new(
                "shell-input",
                "no input acknowledgement within 5 seconds",
            ));
        }
        let acknowledged = output
            .lines()
            .filter_map(|l| l.strip_prefix("RR:CONSOLE:INPUT seq="))
            .next_back()
            .map_or(Ok(0), |s| {
                s.parse::<usize>()
                    .map_err(|_| Error::new("shell-input", "bad sequence"))
            })?;
        if acknowledged > self.sent {
            return Err(Error::new("shell-input", "acknowledged unsent bytes"));
        }
        if acknowledged == self.sent && self.sent < self.input.len() {
            match stream.write(&self.input[self.sent..self.sent + 1]) {
                Ok(1) => {
                    self.sent += 1;
                    self.last_progress = Instant::now()
                }
                Ok(_) => return Err(Error::new("shell-input", "zero write")),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => (),
                Err(e) => return Err(e.into()),
            }
        }
        Ok(false)
    }
}
