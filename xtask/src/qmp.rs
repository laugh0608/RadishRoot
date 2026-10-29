//! 有界 QMP JSON 通道；消息、回复身份和异步事件分别处理，不开放网络监听。
use crate::error::{Error, Result};
use serde_json::{Value, json};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::Path,
    thread,
    time::{Duration, Instant},
};
const MAX_MESSAGE: usize = 64 * 1024;
pub struct Client {
    stream: UnixStream,
    buffer: Vec<u8>,
    id: u64,
    log: File,
}
fn bad(detail: impl Into<String>) -> Error {
    Error::new("qmp", detail)
}
impl Client {
    pub fn connect(socket: &Path, log: &Path) -> Result<Self> {
        let stream = UnixStream::connect(socket)?;
        stream.set_nonblocking(true)?;
        let mut client = Self {
            stream,
            buffer: Vec::new(),
            id: 0,
            log: OpenOptions::new().write(true).create_new(true).open(log)?,
        };
        let greeting = client.read(Instant::now() + Duration::from_secs(3))?;
        validate_greeting(&greeting)?;
        client.execute("qmp_capabilities", json!({}))?;
        Ok(client)
    }
    fn read(&mut self, deadline: Instant) -> Result<Value> {
        loop {
            if let Some(index) = self.buffer.iter().position(|b| *b == b'\n') {
                if index > MAX_MESSAGE {
                    return Err(bad("message exceeds 64 KiB"));
                }
                let bytes = self.buffer.drain(..=index).collect::<Vec<_>>();
                let value: Value =
                    serde_json::from_slice(&bytes).map_err(|e| bad(e.to_string()))?;
                writeln!(self.log, "receive {}", value)?;
                self.log.flush()?;
                return Ok(value);
            }
            if self.buffer.len() > MAX_MESSAGE {
                return Err(bad("message exceeds 64 KiB"));
            }
            if Instant::now() >= deadline {
                return Err(bad("reply deadline exceeded (3 seconds)"));
            }
            let mut bytes = [0; 4096];
            match self.stream.read(&mut bytes) {
                Ok(0) => return Err(bad("channel closed")),
                Ok(n) => self.buffer.extend_from_slice(&bytes[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(2))
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
    pub fn execute(&mut self, command: &str, arguments: Value) -> Result<Value> {
        self.id = self
            .id
            .checked_add(1)
            .ok_or_else(|| bad("request ID overflow"))?;
        let request = json!({"execute":command,"arguments":arguments,"id":self.id});
        let mut bytes = serde_json::to_vec(&request).map_err(|e| bad(e.to_string()))?;
        bytes.push(b'\n');
        if bytes.len() > MAX_MESSAGE {
            return Err(bad("request exceeds 64 KiB"));
        }
        writeln!(self.log, "send {}", request)?;
        self.log.flush()?;
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut sent = 0;
        while sent < bytes.len() {
            if Instant::now() >= deadline {
                return Err(bad("request write deadline"));
            }
            match self.stream.write(&bytes[sent..]) {
                Ok(0) => return Err(bad("zero-byte write")),
                Ok(n) => sent += n,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(2))
                }
                Err(e) => return Err(e.into()),
            }
        }
        loop {
            let response = self.read(deadline)?;
            if let Some(value) = reply(response, self.id)? {
                return Ok(value);
            }
        }
    }
}
fn validate_greeting(value: &Value) -> Result<()> {
    let qmp = value
        .get("QMP")
        .ok_or_else(|| bad("missing QMP greeting"))?;
    if !qmp.get("capabilities").is_some_and(Value::is_array)
        || !qmp.get("version").is_some_and(Value::is_object)
    {
        return Err(bad("invalid QMP greeting"));
    }
    Ok(())
}
fn reply(value: Value, id: u64) -> Result<Option<Value>> {
    if value.get("event").is_some_and(Value::is_string) {
        if value.get("id").is_some()
            || value.get("return").is_some()
            || value.get("error").is_some()
        {
            return Err(bad("mixed event/reply"));
        }
        return Ok(None);
    }
    if value.get("id").and_then(Value::as_u64) != Some(id) {
        return Err(bad("unexpected reply ID"));
    }
    if let Some(error) = value.get("error") {
        return Err(bad(format!("device command rejected: {error}")));
    }
    value
        .get("return")
        .cloned()
        .map(Some)
        .ok_or_else(|| bad("reply missing return/error"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn greeting_reply_ids_errors_and_events_are_not_interchangeable() {
        assert!(validate_greeting(&json!({"QMP":{"version":{},"capabilities":[]}})).is_ok());
        assert!(validate_greeting(&json!({"return":{}})).is_err());
        assert_eq!(reply(json!({"event":"RESET","data":{}}), 1).unwrap(), None);
        assert_eq!(
            reply(json!({"id":1,"return":{}}), 1).unwrap(),
            Some(json!({}))
        );
        for value in [
            json!({"id":2,"return":{}}),
            json!({"id":1,"error":{"desc":"failed"}}),
            json!({"id":1}),
            json!({"event":"RESET","id":1,"return":{}}),
        ] {
            assert!(reply(value, 1).is_err());
        }
    }
}
