//! 真实键鼠驱动工作台，宿主用实际输入记录和同一只读盘重放；独立像素另行核对。
mod cases;
mod demo;
mod link;
mod power;
use crate::{
    error::{Error, Result},
    process::{self, Runner},
    qmp::Client,
    screen::{Ppm, Snapshot},
    vm::{fields, number},
    vm_graphics::read_serial,
};
pub use cases::execute;
pub(super) use demo::DemoBoot;
pub use demo::execute as execute_demo;
pub use link::execute as execute_link;
pub use power::CASES as POWER_CASES;
pub use power::execute as execute_power;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};
fn bad(s: impl Into<String>) -> Error {
    Error::new("workbench-contract", s)
}
fn json_lines(output: &str, prefix: &str) -> Result<Vec<Value>> {
    output
        .lines()
        .filter_map(|line| line.strip_prefix(prefix))
        .map(|line| serde_json::from_str(line).map_err(|e| bad(e.to_string())))
        .collect()
}
pub struct Session {
    serial: PathBuf,
    socket: PathBuf,
    directory: PathBuf,
    client: Option<Client>,
    full: bool,
    gui: bool,
    mode: String,
    console_socket: Option<PathBuf>,
    deadline: Instant,
    initial: bool,
    power_done: bool,
    pub(super) demo: Option<DemoBoot>,
    pub snapshots: Vec<Snapshot>,
}
impl Session {
    pub fn new(
        serial: &Path,
        socket: &Path,
        directory: &Path,
        full: bool,
        gui: bool,
        mode: &str,
        console_socket: Option<&Path>,
    ) -> Self {
        Self {
            serial: serial.into(),
            socket: socket.into(),
            directory: directory.into(),
            client: None,
            full,
            gui,
            mode: mode.into(),
            console_socket: console_socket.map(PathBuf::from),
            deadline: Instant::now() + Duration::from_secs(if gui { 600 } else { 60 }),
            initial: false,
            power_done: false,
            demo: None,
            snapshots: vec![],
        }
    }
    fn state(&self) -> Result<String> {
        read_serial(&self.serial)?
            .split_inclusive('\n')
            .rfind(|l| l.ends_with('\n') && l.starts_with("RR:T10:FRAME "))
            .map(|s| s.trim_end().into())
            .ok_or_else(|| bad("no workbench frame"))
    }
    fn wait(&self, predicate: impl Fn(&str) -> Result<bool>) -> Result<String> {
        let deadline = (Instant::now() + Duration::from_secs(3)).min(self.deadline);
        loop {
            let serial = read_serial(&self.serial)?;
            if serial.contains(":FAIL ") {
                return Err(bad("guest failure during workbench input"));
            }
            if let Ok(s) = self.state()
                && predicate(&s)?
            {
                return Ok(s);
            }
            if Instant::now() >= deadline {
                return Err(bad("workbench input deadline"));
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
    fn send(&mut self, events: serde_json::Value) -> Result<()> {
        self.client
            .as_mut()
            .ok_or_else(|| bad("missing QMP"))?
            .execute("input-send-event", json!({"events":events}))?;
        Ok(())
    }
    fn key(&mut self, key: &str, down: bool) -> Result<()> {
        let before = self.state()?;
        let before = fields(&before)?;
        let counter = if down { "events" } else { "releases" };
        let n = number(&before, counter)?;
        self.send(json!([{"type":"key","data":{"down":down,"key":{"type":"qcode","data":key}}}]))?;
        self.wait(|s| Ok(number(&fields(s)?, counter)? > n))?;
        Ok(())
    }
    fn tap(&mut self, key: &str) -> Result<()> {
        self.key(key, true)?;
        self.key(key, false)
    }
    fn command(&mut self, line: &str, count: u64) -> Result<()> {
        for c in line.chars() {
            let s = c.to_ascii_lowercase().to_string();
            let key = match c {
                ' ' => "spc",
                '.' => "dot",
                '-' => "minus",
                '/' => "slash",
                '"' => "apostrophe",
                ':' => "semicolon",
                _ => s.as_str(),
            };
            let shifted = c.is_ascii_uppercase() || matches!(c, '"' | ':');
            if shifted {
                self.key("shift", true)?;
            }
            self.tap(key)?;
            if shifted {
                self.key("shift", false)?;
            }
        }
        self.tap("ret")?;
        self.wait(|s| {
            let f = fields(s)?;
            Ok(number(&f, "results")? == count && number(&f, "pending")? == 0)
        })?;
        Ok(())
    }
    fn click(&mut self, x: u64, y: u64) -> Result<()> {
        loop {
            let s = self.state()?;
            let f = fields(&s)?;
            let (sx, sy) = (number(&f, "x")?, number(&f, "y")?);
            if (sx, sy) == (x, y) {
                break;
            }
            let dx = (x as i64 - sx as i64).clamp(-100, 100);
            let dy = (y as i64 - sy as i64).clamp(-100, 100);
            self.send(json!([{"type":"rel","data":{"axis":"x","value":dx}},{"type":"rel","data":{"axis":"y","value":dy}}]))?;
            self.wait(|s| {
                let f = fields(s)?;
                Ok(number(&f, "x")? == (sx as i64 + dx) as u64
                    && number(&f, "y")? == (sy as i64 + dy) as u64)
            })?;
        }
        self.send(json!([{"type":"btn","data":{"button":"left","down":true}}]))?;
        self.wait(|s| Ok(number(&fields(s)?, "left")? == 1))?;
        self.send(json!([{"type":"btn","data":{"button":"left","down":false}}]))?;
        self.wait(|s| Ok(number(&fields(s)?, "left")? == 0))?;
        Ok(())
    }
    fn snapshot(&mut self, name: &str) -> Result<()> {
        let serial = read_serial(&self.serial)?;
        let frame = self.state()?;
        let path = self.directory.join(format!("{name}.ppm"));
        if path.exists() {
            return Err(bad("screenshot exists"));
        }
        // 保留本帧前实际消费的输入 / 服务步骤，宿主只重放此记录。
        let mut replay = String::new();
        for line in serial.lines() {
            if let Some(step) = line.strip_prefix("RR:T10:STEP ") {
                replay.push_str(step);
                replay.push('\n');
            }
            if line == frame {
                break;
            }
        }
        fs::write(self.directory.join(format!("{name}.steps")), replay)?;
        self.client
            .as_mut()
            .ok_or_else(|| bad("missing QMP"))?
            .execute("screendump", json!({"filename":path,"format":"ppm"}))?;
        self.snapshots.push(Snapshot {
            name: name.into(),
            frame,
            path,
        });
        Ok(())
    }
    pub fn poll(&mut self) -> Result<bool> {
        if Instant::now() >= self.deadline {
            return Err(bad("workbench case deadline"));
        }
        let serial = read_serial(&self.serial)?;
        if serial.contains(":FAIL ") && serial.ends_with('\n') {
            return Err(bad("guest failure before workbench ready"));
        }
        if self.client.is_none() && self.socket.exists() {
            self.client = Some(Client::connect(
                &self.socket,
                &self.directory.join("qmp.jsonl"),
            )?);
        }
        if !serial.contains("RR:T10:READY ") || self.client.is_none() {
            return Ok(false);
        }
        let Ok(state) = self.state() else {
            return Ok(false);
        };
        if number(&fields(&state)?, "pending")? != 0 {
            return Ok(false);
        }
        if !self.initial {
            self.snapshot("workbench-initial")?;
            self.initial = true;
        }
        if self.demo.is_some() {
            return self.demo_poll();
        }
        if self.mode.starts_with("t12-") {
            return self.power_poll();
        }
        if self.gui {
            let f = fields(&state)?;
            if number(&f, "releases")? < 2
                || number(&f, "button_releases")? < 1
                || number(&f, "results")? < 1
            {
                return Ok(false);
            }
            self.snapshot("workbench-window")?;
        } else if self.mode.starts_with("t11-") {
            self.link_steps()?;
        } else if !self.mode.is_empty() {
            self.case_steps()?;
        } else if self.full {
            self.command("help", 1)?;
            self.snapshot("workbench-help")?;
            self.command("notacommand", 2)?;
            self.snapshot("workbench-parse-error")?;
            self.command("write denied.txt x", 3)?;
            self.snapshot("workbench-readonly")?;
            self.click(590, 116)?;
            self.wait(|s| Ok(number(&fields(s)?, "details")? == 1))?;
            self.snapshot("workbench-details")?;
            self.tap("esc")?;
            self.tap("tab")?;
            self.tap("ret")?;
            self.snapshot("workbench-keyboard-focus")?;
        }
        let generation = number(&fields(&self.state()?)?, "generation")?;
        thread::sleep(Duration::from_millis(550));
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
            return Err(bad("workbench did not become idle"));
        }
        Ok(true)
    }
}
fn independent(image: &Ppm, frame: &str) -> Result<()> {
    let f = fields(frame)?;
    let (w, h) = (
        number(&f, "width")? as usize,
        number(&f, "height")? as usize,
    );
    let (x, y) = (number(&f, "x")? as usize, number(&f, "y")? as usize);
    if (w, h) != (1024, 768) || image.width != w || image.height != h || x >= w || y >= h {
        return Err(bad("workbench screenshot geometry"));
    }
    let pixel = |x: usize, y: usize| &image.pixels[(y * w + x) * 3..(y * w + x) * 3 + 3];
    if pixel(x, y) != [32, 47, 42] || ((x, y) != (0, 0) && pixel(0, 0) != [239, 242, 239]) {
        return Err(bad("workbench pointer/background pixel"));
    }
    // 内建 10x20 R 的独立 BDF 行；工作台标题左上为 (12,8)。
    for (i, bit) in [0, 1, 1, 1, 1, 1, 1, 0, 0, 0].into_iter().enumerate() {
        if pixel(12 + i, 11)
            != if bit == 1 {
                [32, 47, 42]
            } else {
                [239, 242, 239]
            }
        {
            return Err(bad("workbench independent title glyph"));
        }
    }
    Ok(())
}
pub fn verify(
    runner: &mut Runner,
    host: &Path,
    directory: &Path,
    disk: &Path,
    boot: [u8; 16],
    mut session: Session,
    output: &str,
) -> Result<()> {
    validate(output, session.full && session.mode.is_empty(), session.gui)?;
    let exit_reason = if let Some(demo) = &session.demo {
        demo::validate(output, demo, boot)?;
        Some("guest-shutdown")
    } else if session.mode.starts_with("t12-") {
        power::validate(output, &session.mode)?;
        (!power::failure(&session.mode)).then_some(if session.mode == "t12-reboot" {
            "guest-reset"
        } else {
            "guest-shutdown"
        })
    } else if session.mode.starts_with("t11-") {
        link::validate(output, &session.mode)?;
        None
    } else {
        if !session.mode.is_empty() {
            cases::validate(output, &session.mode)?;
        }
        None
    };
    if let Some(reason) = exit_reason {
        session
            .client
            .as_mut()
            .ok_or_else(|| bad("missing QMP exit observer"))?
            .guest_exit(reason)?;
        runner.record("power_exit=natural qmp_guest_reason=verified")?;
    }
    if output
        .lines()
        .filter(|l| l.starts_with("RR:T10:READY "))
        .count()
        != 1
        || output.contains("RR:T07:FRAME ")
    {
        return Err(bad("workbench ready identity"));
    }
    let mut report = vec![];
    for snapshot in session.snapshots {
        let image = Ppm::read(&snapshot.path)?;
        independent(&image, &snapshot.frame)?;
        let compare_host =
            session.demo.is_none() && (session.mode.is_empty() || session.mode == "t10-pages");
        if compare_host {
            let dir = process::unique_directory(directory, "workbench-host")?;
            runner.run(
                Command::new(host)
                    .arg(&dir)
                    .arg("--workbench-disk")
                    .arg(disk)
                    .arg("--boot-id")
                    .arg(format!("{:032x}", u128::from_be_bytes(boot)))
                    .arg("--replay")
                    .arg(directory.join(format!("{}.steps", snapshot.name))),
            )?;
            let shared = Ppm::read(&dir.join("frame.ppm"))?;
            if image.width != shared.width
                || image.height != shared.height
                || image.pixels != shared.pixels
            {
                return Err(bad(format!(
                    "host/guest workbench mismatch: {}",
                    snapshot.name
                )));
            }
        }
        let mut damaged = Ppm {
            width: image.width,
            height: image.height,
            pixels: image.pixels.clone(),
        };
        let f = fields(&snapshot.frame)?;
        let x = number(&f, "x")? as usize;
        let y = number(&f, "y")? as usize;
        damaged.pixels[(y * image.width + x) * 3] ^= 1;
        if independent(&damaged, &snapshot.frame).is_ok() {
            return Err(bad("damaged workbench screenshot accepted"));
        }
        crate::digest(&snapshot.path, runner)?;
        report.push(json!({"name":snapshot.name,"frame":snapshot.frame,"independent_pixels":true,"host_same_view":compare_host,"host_comparison_scope":"readonly source only; writable cases have independent DATA verification","damaged_pixel_rejected":true}));
    }
    fs::write(
        directory.join("workbench-screen-report.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| bad(e.to_string()))?,
    )?;
    Ok(())
}

fn validate(output: &str, full: bool, gui: bool) -> Result<()> {
    let frames = output
        .lines()
        .filter(|l| l.starts_with("RR:T10:FRAME "))
        .map(fields)
        .collect::<Result<Vec<_>>>()?;
    if frames.len() < 2 {
        return Err(bad("missing workbench initial / ready frames"));
    }
    let mut events = 0;
    let mut releases = 0;
    for (i, f) in frames.iter().enumerate() {
        if number(f, "generation")? != i as u64 + 1
            || number(f, "events")? < events
            || number(f, "releases")? < releases
            || number(f, "heap")? > 1024 * 1024
            || number(f, "entries")? > 32
            || number(f, "results")? > 16
        {
            return Err(bad("workbench frame sequence / budget"));
        }
        events = number(f, "events")?;
        releases = number(f, "releases")?;
    }
    let replies = output
        .lines()
        .filter_map(|l| l.strip_prefix("RR:T10:REPLY "))
        .map(|s| serde_json::from_str::<serde_json::Value>(s).map_err(|e| bad(e.to_string())))
        .collect::<Result<Vec<_>>>()?;
    if full {
        if replies.len() != 3
            || replies[0]["ok"] != true
            || replies[0]["output"]["kind"] != "help"
            || replies[1]["ok"] != false
            || replies[1]["error"]["kind"] != "parse"
            || replies[1]["error"]["code"] != "UnknownCommand"
            || !replies[1]["operation_id"].is_null()
            || replies[2]["ok"] != false
            || replies[2]["error"]["detail"]["code"] != "ReadOnly"
            || replies[2]["error"]["detail"]["stage"] != "Validate"
            || replies[2]["error"]["detail"]["object"] != "/data/denied.txt"
            || replies[2]["error"]["detail"]["partial_effect"]["may_have_changed"] != false
        {
            return Err(bad(
                "graphical command result differs from independent expected facts",
            ));
        }
        let details = output
            .lines()
            .filter_map(|l| l.strip_prefix("RR:T10:DETAIL "))
            .map(|s| serde_json::from_str::<serde_json::Value>(s).map_err(|e| bad(e.to_string())))
            .collect::<Result<Vec<_>>>()?;
        if details.len() != 1
            || details[0]["ok"] != true
            || details[0]["output"]["record"]["operation_id"] != replies[2]["operation_id"]
            || details[0]["output"]["record"]["status"] != "Failed"
            || details[0]["output"]["record"]["error"]["code"] != "ReadOnly"
        {
            return Err(bad(
                "graphical failure detail did not retain operation identity / facts",
            ));
        }
    }
    if (full || gui) && (releases < 2 || number(frames.last().unwrap(), "button_releases")? < 1) {
        return Err(bad("missing actual keyboard / pointer release"));
    }
    Ok(())
}
