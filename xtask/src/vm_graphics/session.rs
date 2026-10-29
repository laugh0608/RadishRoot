use super::{bad, read_serial};
use crate::{
    error::Result,
    qmp::Client,
    screen::Snapshot,
    vm::{fields, number},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

pub struct State {
    pub line: String,
    values: BTreeMap<String, String>,
}
impl State {
    fn parse(line: &str) -> Result<Self> {
        Ok(Self {
            line: line.into(),
            values: fields(line)?
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        })
    }
    pub fn n(&self, key: &str) -> Result<u64> {
        self.values
            .get(key)
            .ok_or_else(|| bad(format!("missing frame field {key}")))?
            .parse()
            .map_err(|_| bad(format!("bad frame number {key}")))
    }
    pub fn s(&self, key: &str) -> Result<&str> {
        self.values
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| bad(format!("missing frame field {key}")))
    }
}
pub struct Session {
    serial: PathBuf,
    socket: PathBuf,
    directory: PathBuf,
    mode: String,
    full: bool,
    gui: bool,
    client: Option<Client>,
    deadline: Instant,
    initial: bool,
    pub snapshots: Vec<Snapshot>,
}
impl Session {
    pub fn new(
        serial: &Path,
        socket: &Path,
        directory: &Path,
        mode: &str,
        full: bool,
        gui: bool,
    ) -> Self {
        Self {
            serial: serial.into(),
            socket: socket.into(),
            directory: directory.into(),
            mode: mode.into(),
            full,
            gui,
            client: None,
            deadline: Instant::now() + Duration::from_secs(if gui { 600 } else { 60 }),
            initial: false,
            snapshots: Vec::new(),
        }
    }
    fn alive(&self) -> Result<()> {
        if Instant::now() >= self.deadline {
            return Err(bad("case deadline reached"));
        }
        Ok(())
    }
    pub fn poll(&mut self) -> Result<bool> {
        self.alive()?;
        let serial = read_serial(&self.serial)?;
        if serial.lines().any(|l| {
            l.starts_with("RR:")
                && l.contains(":FAIL ")
                && l.ends_with(|c: char| !c.is_whitespace())
        }) {
            // 终态行必须完整，后续 validate 核对该用例的精确失败原因。
            if serial.ends_with('\n') {
                if self.client.is_some() && self.state()?.is_some() && self.snapshots.is_empty() {
                    self.snapshot("failure")?;
                }
                return Ok(true);
            }
            return Ok(false);
        }
        if self.client.is_none() && self.socket.exists() {
            self.client = Some(Client::connect(
                &self.socket,
                &self.directory.join("qmp.jsonl"),
            )?);
        }
        if !serial.contains("RR:T07:READY ") || !serial.ends_with('\n') || self.client.is_none() {
            return Ok(false);
        }
        if !self.initial {
            self.snapshot("initial")?;
            self.initial = true;
        }
        if self.gui {
            let state = self.state()?.ok_or_else(|| bad("GUI missing frame"))?;
            if state.n("releases")? < 2
                || state.n("button_releases")? < 1
                || state.n("activate0")? + state.n("activate1")? < 1
            {
                return Ok(false);
            }
            self.snapshot("window-interaction")?;
            return Ok(true);
        }
        match self.mode.as_str() {
            "t07-corrupt-input" => self.recovery(false)?,
            "t07-input-overflow" => self.recovery(true)?,
            "t07-redraw" => self.redraw()?,
            "" if self.full => self.normal()?,
            "t07-resolution" => {
                self.key("a", true)?;
                self.key("a", false)?;
                self.move_to(0, 0)?;
                self.move_to(799, 599)?;
                self.snapshot("resolution-interaction")?;
            }
            "t07-clipping" => {}
            "" => {}
            _ => return Err(bad("unexpected READY for failure case")),
        }
        self.idle_stability()?;
        Ok(true)
    }
    fn state(&self) -> Result<Option<State>> {
        let serial = read_serial(&self.serial)?;
        serial
            .split_inclusive('\n')
            .rfind(|l| l.ends_with('\n') && l.starts_with("RR:T07:FRAME "))
            .map(|l| State::parse(l.trim_end()))
            .transpose()
    }
    fn wait(&self, predicate: impl Fn(&State) -> Result<bool>) -> Result<State> {
        let deadline = (Instant::now() + Duration::from_secs(3)).min(self.deadline);
        loop {
            self.alive()?;
            let serial = read_serial(&self.serial)?;
            if serial.contains("RR:T07:FAIL") || serial.contains("RR:T07:OFFLINE") {
                return Err(bad("guest failed while processing input"));
            }
            if let Some(state) = self.state()?
                && predicate(&state)?
            {
                return Ok(state);
            }
            if Instant::now() >= deadline {
                return Err(bad("guest state deadline (3 seconds)"));
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
    fn wait_line(&self, prefix: &str) -> Result<String> {
        let deadline = (Instant::now() + Duration::from_secs(3)).min(self.deadline);
        loop {
            self.alive()?;
            let serial = read_serial(&self.serial)?;
            if let Some(line) = serial
                .split_inclusive('\n')
                .filter(|l| l.ends_with('\n'))
                .find(|l| l.starts_with(prefix))
            {
                return Ok(line.trim_end().into());
            }
            if Instant::now() >= deadline {
                return Err(bad(format!("missing {prefix}")));
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
    fn send(&mut self, events: serde_json::Value) -> Result<()> {
        self.alive()?;
        self.client
            .as_mut()
            .ok_or_else(|| bad("QMP not connected"))?
            .execute("input-send-event", json!({"events":events}))?;
        Ok(())
    }
    fn key(&mut self, key: &str, down: bool) -> Result<State> {
        let before = self
            .state()?
            .ok_or_else(|| bad("missing frame before key"))?;
        self.send(json!([{"type":"key","data":{"down":down,"key":{"type":"qcode","data":key}}}]))?;
        self.wait(|s| {
            Ok(if down {
                s.n("events")? > before.n("events")?
            } else {
                s.n("releases")? > before.n("releases")?
            })
        })
    }
    fn tap(&mut self, key: &str) -> Result<State> {
        self.key(key, true)?;
        self.key(key, false)
    }
    fn move_to(&mut self, x: u64, y: u64) -> Result<State> {
        let mut state = self
            .state()?
            .ok_or_else(|| bad("missing frame before move"))?;
        while state.n("x")? != x || state.n("y")? != y {
            let dx = (x as i64 - state.n("x")? as i64).clamp(-100, 100);
            let dy = (y as i64 - state.n("y")? as i64).clamp(-100, 100);
            let expected_x = (state.n("x")? as i64 + dx) as u64;
            let expected_y = (state.n("y")? as i64 + dy) as u64;
            self.send(json!([{"type":"rel","data":{"axis":"x","value":dx}},{"type":"rel","data":{"axis":"y","value":dy}}]))?;
            state = self.wait(|s| Ok(s.n("x")? == expected_x && s.n("y")? == expected_y))?;
        }
        Ok(state)
    }
    fn button(&mut self, down: bool) -> Result<State> {
        let before = self
            .state()?
            .ok_or_else(|| bad("missing frame before button"))?;
        let counter = if down {
            "button_presses"
        } else {
            "button_releases"
        };
        self.send(json!([{"type":"btn","data":{"button":"left","down":down}}]))?;
        self.wait(|s| Ok(s.n(counter)? > before.n(counter)? && s.n("left")? == u64::from(down)))
    }
    fn snapshot(&mut self, name: &str) -> Result<()> {
        self.alive()?;
        let state = self
            .state()?
            .ok_or_else(|| bad("missing frame for screenshot"))?;
        let path = self.directory.join(format!("{name}.ppm"));
        if path.exists() {
            return Err(bad("screenshot path already exists"));
        }
        self.client
            .as_mut()
            .ok_or_else(|| bad("missing QMP"))?
            .execute("screendump", json!({"filename":path,"format":"ppm"}))?;
        self.snapshots.push(Snapshot {
            name: name.into(),
            frame: state.line,
            path,
        });
        Ok(())
    }
    fn normal(&mut self) -> Result<()> {
        self.tap("a")?;
        self.key("shift", true)?;
        self.tap("a")?;
        self.key("shift", false)?;
        self.tap("caps_lock")?;
        let upper = self.tap("a")?;
        self.tap("caps_lock")?;
        if upper.s("text")? != "614141" {
            return Err(bad("Shift/Caps text differs from aAA"));
        }
        for modifier in ["ctrl", "alt"] {
            self.key(modifier, true)?;
            self.tap("a")?;
            let s = self.key(modifier, false)?;
            if s.s("text")? != "614141" {
                return Err(bad("control combination inserted text"));
            }
        }
        self.tap("tab")?;
        let activated = self.tap("ret")?;
        if activated.n("focus")? != 1 || activated.n("activate1")? != 1 {
            return Err(bad("Tab/Enter activation failed"));
        }
        self.snapshot("keyboard")?;
        let w = activated.n("width")?;
        let h = activated.n("height")?;
        for (index, (x, y)) in [(0, 0), (w - 1, 0), (w - 1, h - 1), (0, h - 1)]
            .into_iter()
            .enumerate()
        {
            self.move_to(x, y)?;
            // 再向边界外送一个实际包，确认 clamp 后状态仍在画面内。
            let before = self.state()?.unwrap().n("events")?;
            self.send(json!([{"type":"rel","data":{"axis":"x","value":if x==0{-50}else{50}}},{"type":"rel","data":{"axis":"y","value":if y==0{-50}else{50}}}]))?;
            self.wait(|s| Ok(s.n("events")? > before && s.n("x")? == x && s.n("y")? == y))?;
            self.snapshot(&format!("edge-{index}"))?;
        }
        self.move_to(30, 160)?;
        self.button(true)?;
        let clicked = self.button(false)?;
        if clicked.n("focus")? != 0 {
            return Err(bad("pointer focus failed"));
        }
        self.tap("esc")?;
        for _ in 0..129 {
            self.tap("x")?;
        }
        let full = self.state()?.unwrap();
        if full.s("text")? != "78".repeat(128) || full.s("status")? != "full" {
            return Err(bad("text capacity rejection failed"));
        }
        self.snapshot("text-limit")?;
        self.tap("backspace")?;
        self.tap("esc")?;
        self.tap("a")?;
        self.move_to(w / 2, h / 2 + 30)?;
        self.snapshot("restored")?;
        Ok(())
    }
    fn recovery(&mut self, overflow: bool) -> Result<()> {
        self.key("shift", true)?;
        if overflow {
            self.wait_line("RR:T07:PAUSE ")?;
            // 一批两个完整 make/break（6 个 Set 2 字节），低于设备 FIFO 容量；
            // 必须看到 guest 实际接收计数前进，不能把 QMP 请求数当成到达数。
            let mut received = self.state()?.unwrap().n("raw_received")?;
            for _ in 0..64 {
                let mut events = Vec::new();
                for _ in 0..2 {
                    for down in [true, false] {
                        events.push(json!({"type":"key","data":{"down":down,"key":{"type":"qcode","data":"a"}}}));
                    }
                }
                self.send(json!(events))?;
                let expected = received + 6;
                let deadline = (Instant::now() + Duration::from_secs(3)).min(self.deadline);
                loop {
                    self.alive()?;
                    let serial = read_serial(&self.serial)?;
                    if serial.contains("RR:T07:RESET ") {
                        break;
                    }
                    let latest = serial
                        .split_inclusive('\n')
                        .filter(|line| line.ends_with('\n'))
                        .rev()
                        .find(|line| {
                            line.starts_with("RR:T07:IDLE ") || line.starts_with("RR:T07:FRAME ")
                        });
                    if let Some(line) = latest {
                        received = number(&fields(line)?, "raw_received")?;
                        if received >= expected {
                            break;
                        }
                    }
                    if Instant::now() >= deadline {
                        return Err(bad("raw input batch did not arrive"));
                    }
                    thread::sleep(Duration::from_millis(5));
                }
                if read_serial(&self.serial)?.contains("RR:T07:RESET ") {
                    break;
                }
            }
        }
        let reset = self.wait_line("RR:T07:RESET ")?;
        if overflow && number(&fields(&reset)?, "raw_dropped")? == 0 {
            return Err(bad("overflow did not reach software queue"));
        }
        self.wait_line("RR:T07:RECOVERED ")?;
        self.wait(|s| {
            Ok(s.n("resets")? == 1
                && s.n("shift")? == 0
                && s.n("left")? == 0
                && s.s("text")? == "-")
        })?;
        self.key("shift", false)?;
        let after = self.tap("a")?;
        if after.s("text")? != "61" || after.n("resets")? != 1 || after.n("shift")? != 0 {
            return Err(bad("fresh input after loss did not recover"));
        }
        self.snapshot("recovered")?;
        Ok(())
    }
    fn redraw(&mut self) -> Result<()> {
        let before = self.state()?.unwrap();
        for _ in 0..100 {
            self.tap("a")?;
        }
        let after = self.state()?.unwrap();
        if after.n("generation")? < before.n("generation")? + 100
            || after.n("ticks")? <= before.n("ticks")?
            || after.n("frame_bytes")? != before.n("frame_bytes")?
            || after.n("heap")? != before.n("heap")?
        {
            return Err(bad("redraw progress/accounting"));
        }
        self.snapshot("redraw-100")?;
        Ok(())
    }
    fn idle_stability(&self) -> Result<()> {
        let generation = self
            .state()?
            .ok_or_else(|| bad("missing idle frame"))?
            .n("generation")?;
        let started = Instant::now();
        while started.elapsed() < Duration::from_millis(550) {
            self.alive()?;
            thread::sleep(Duration::from_millis(10));
        }
        let serial = read_serial(&self.serial)?;
        let idle = serial
            .lines()
            .filter(|l| l.starts_with("RR:T07:IDLE "))
            .rev()
            .take(2)
            .map(fields)
            .collect::<Result<Vec<_>>>()?;
        if idle.len() != 2
            || idle.iter().any(|f| {
                number(f, "generation").ok() != Some(generation)
                    || number(f, "pending").ok() != Some(0)
            })
        {
            return Err(bad("idle redraw or unconsumed input"));
        }
        Ok(())
    }
}
