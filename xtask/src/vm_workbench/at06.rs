//! AT06 的工作台入口对照：真实 PS/2 回执、图形拒绝、结构化回复与独立读盘。
use super::{Ppm, Session, Snapshot, bad, cases, fields, json_lines, key_event, number};
use crate::{
    boot_image::Manifest,
    config::Context,
    error::Result,
    process::Runner,
    vm::{self, BootSpec, Machine},
    vm_shell::at06::{self, Event},
    vm_storage,
};
use serde_json::{Value, json};
use std::path::Path;

pub(crate) const MODE: &str = "t10-at06";
const OVERFLOW_FRAME: &str = "at06-line-limit";
const CLEARED_FRAME: &str = "at06-line-cleared";
const RECOVERY_FRAME: &str = "at06-recovered";

pub(crate) fn execute(
    context: &Context,
    runner: &mut Runner,
    manifest: &Manifest,
    machine: &Machine,
    source: &Path,
) -> Result<()> {
    let host = at06::host_trace(context, runner)?;
    let disk = vm_storage::clone_sparse(source, runner)?;
    let serial = vm::boot(
        runner,
        manifest,
        machine,
        BootSpec {
            boot_disk: source,
            data_disk: Some(&disk),
            readonly: false,
            mode: MODE,
        },
    )?;
    compare_replies(&serial, &host)?;
    at06::verify_disk(context, runner, manifest, source, &disk)?;
    runner.record("at06_workbench=pass keyboard_bytes=exact replies=8 overflow=discarded independent_DATA=pass outside_DATA=unchanged")?;
    println!("AT06 workbench: keyboard input, command replies and independent DATA passed");
    Ok(())
}

impl Session {
    pub(super) fn at06_steps(&mut self) -> Result<()> {
        let input = at06::input();
        let lines = std::str::from_utf8(&input)
            .map_err(|_| bad("AT06 fixture encoding"))?
            .split_terminator('\r');
        let mut replies = 0;
        for (index, line) in lines.enumerate() {
            match index {
                3 | 5 => {
                    let (key, n) = if index == 3 { ('x', 1024) } else { ('y', 1025) };
                    if line.len() != n || !line.ends_with(key) {
                        return Err(bad("AT06 fixture boundary"));
                    }
                    let prefix = line.trim_end_matches(key);
                    self.type_text(prefix)?;
                    self.repeat_at06_key(&key.to_string(), line.len() - prefix.len())?;
                    let state = self.state()?;
                    let frame = fields(&state)?;
                    if number(&frame, "input_bytes")? != 1024
                        || (frame.get("notice") == Some(&"InputFull")) != (index == 5)
                    {
                        return Err(bad("AT06 draft size or overflow notice"));
                    }
                    if index == 5 {
                        self.snapshot(OVERFLOW_FRAME)?;
                    }
                    self.tap("ret")?;
                    replies += u64::from(index == 3);
                    self.wait(|state| {
                        let f = fields(state)?;
                        Ok(number(&f, "results")? == replies
                            && number(&f, "input_bytes")? == 0
                            && number(&f, "pending")? == 0)
                    })?;
                    if index == 5 {
                        self.snapshot(CLEARED_FRAME)?;
                    }
                }
                _ => {
                    replies += 1;
                    self.command(line, replies)?;
                    if index == 6 {
                        self.snapshot(RECOVERY_FRAME)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn repeat_at06_key(&mut self, key: &str, mut count: usize) -> Result<()> {
        while count > 0 {
            // 四次普通键入最多产生 12 个 PS/2 字节和 12 个解码事件。
            let batch = count.min(4);
            let state = self.state()?;
            let before = fields(&state)?;
            let releases = number(&before, "releases")? + batch as u64;
            let events = number(&before, "events")? + 3 * batch as u64;
            let pair = [key_event(key, true), key_event(key, false)];
            self.send(Value::Array(
                (0..batch).flat_map(|_| pair.iter().cloned()).collect(),
            ))?;
            self.wait(|state| {
                let f = fields(state)?;
                if number(&f, "resets")? != 0
                    || number(&f, "events")? > events
                    || number(&f, "releases")? > releases
                {
                    return Err(bad("AT06 unexpected keyboard events"));
                }
                Ok(number(&f, "releases")? == releases && number(&f, "events")? == events)
            })?;
            count -= batch;
        }
        Ok(())
    }
}

pub(super) fn validate_input(output: &str, boot: [u8; 16], snapshots: &[Snapshot]) -> Result<()> {
    cases::live_device(output, false)?;
    let initial = format!(
        "RR:T08:DEFAULT entries=0 more=0 readonly=0 generation=0 operation=1 boot_id={:032x}",
        u128::from_be_bytes(boot)
    );
    if output.lines().filter(|line| *line == initial).count() != 1 {
        return Err(bad("AT06 workbench initial boot or volume facts"));
    }
    let mut input = Vec::new();
    for line in output
        .lines()
        .filter_map(|s| s.strip_prefix("RR:T10:STEP "))
    {
        if let Some(text) = line.strip_prefix("text ") {
            input.push(text.parse::<u8>().map_err(|_| bad("AT06 keyboard byte"))?);
        } else if line == "key enter 0 0" {
            input.push(b'\r');
        } else if line == "reset" {
            return Err(bad("AT06 keyboard reset"));
        }
    }
    if input != at06::input() || output.contains("RR:T07:RESET ") {
        return Err(bad("AT06 guest keyboard receipt differs from host fixture"));
    }
    for (name, bytes, count, show_notice) in [
        (OVERFLOW_FRAME, 1024, 5, Some(true)),
        (CLEARED_FRAME, 0, 5, None),
        (RECOVERY_FRAME, 0, 6, Some(false)),
    ] {
        let snapshot = snapshots
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| bad(format!("missing AT06 snapshot {name}")))?;
        let f = fields(&snapshot.frame)?;
        if number(&f, "input_bytes")? != bytes
            || number(&f, "results")? != count
            || number(&f, "pending")? != 0
        {
            return Err(bad(format!("wrong AT06 snapshot state: {name}")));
        }
        let Some(show_notice) = show_notice else {
            continue;
        };
        let notice = if show_notice { "InputFull" } else { "Ready" };
        if f.get("notice") != Some(&notice) {
            return Err(bad(format!("wrong AT06 status notice: {name}")));
        }
        let image = Ppm::read(&snapshot.path)?;
        if (image.width, image.height) != (1024, 768) {
            return Err(bad("AT06 status screenshot size"));
        }
        // 1024×768 状态行的文字像素，避开下方文件区和终端中的错误颜色。
        let status = &image.pixels[60 * image.width * 3..80 * image.width * 3];
        let has_error_text = status
            .as_chunks::<3>()
            .0
            .iter()
            .filter(|&&pixel| pixel == [161, 44, 42])
            .count()
            > 20;
        if has_error_text != show_notice {
            return Err(bad(format!("wrong AT06 status pixels: {name}")));
        }
    }
    Ok(())
}

fn compare_replies(output: &str, host: &[Event]) -> Result<()> {
    let host: Vec<_> = host
        .iter()
        .filter_map(|event| match event {
            Event::Reply(reply) => Some(reply),
            Event::Capacity => None,
        })
        .collect();
    let mut guest = json_lines(output, "RR:T10:REPLY ")?;
    if host.len() != 8 || guest.len() != host.len() {
        return Err(bad("AT06 workbench reply count"));
    }
    let defaults: Vec<_> = output
        .lines()
        .filter(|line| line.starts_with("RR:T08:DEFAULT "))
        .collect();
    let [initial] = defaults.as_slice() else {
        return Err(bad("AT06 initial storage query"));
    };
    let initial = fields(initial)?;
    let boot = initial
        .get("boot_id")
        .filter(|id| at06::valid_volume(id))
        .ok_or_else(|| bad("AT06 workbench boot identity"))?;
    let volume = guest[3]["output"]["reference"]["volume"]
        .as_str()
        .filter(|id| at06::valid_volume(id))
        .ok_or_else(|| bad("AT06 workbench volume identity"))?
        .to_owned();
    let pages: Vec<_> = output
        .lines()
        .filter(|line| line.starts_with("RR:T11:PAGE "))
        .collect();
    if number(&initial, "entries")? != 0
        || number(&initial, "operation")? != 1
        || pages
            != [
                "RR:T11:PAGE query=2 generation=0 source=0 update=idle ok=1",
                "RR:T11:PAGE query=5 generation=1 source=4 update=refreshed ok=1",
            ]
    {
        return Err(bad("AT06 background list and refresh evidence"));
    }
    for reply in &mut guest {
        at06::normalize(reply, boot, &volume)?;
    }
    let records = guest[7]["output"]["records"]
        .as_array_mut()
        .filter(|records| records.len() == 8)
        .ok_or_else(|| bad("AT06 workbench service record count"))?;
    for (i, record) in records.iter().enumerate() {
        if record["operation_id"] != format!("<boot>:{}", i + 1) {
            return Err(bad("AT06 workbench service record order"));
        }
    }
    // 校验独立读操作的完整记录，再从用户命令对照中移除。
    for index in [4, 1, 0] {
        let mut read = records.remove(index);
        read["operation_id"] = json!("<boot>:1");
        if read != host[7]["output"]["records"][0] {
            return Err(bad("AT06 background list record differs"));
        }
    }
    for (index, record) in records.iter_mut().enumerate() {
        record["operation_id"] = json!(format!("<boot>:{}", index + 1));
    }
    for (index, (reply, seq)) in guest.iter_mut().zip([3, 0, 0, 4, 6, 7, 8, 0]).enumerate() {
        let actual = reply
            .get_mut("operation_id")
            .ok_or_else(|| bad("AT06 missing graphical operation identity field"))?;
        if seq > 0 {
            if *actual != format!("<boot>:{seq}") {
                return Err(bad("AT06 graphical command operation order"));
            }
            *actual = host[index]["operation_id"].clone();
        }
        if reply != host[index] {
            return Err(bad(format!("AT06 graphical reply {} differs", index + 1)));
        }
    }
    Ok(())
}
