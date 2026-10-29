mod session;
use crate::{
    boot_image::{self, Manifest},
    config::Context,
    error::{Error, Result},
    process::Runner,
    vm::{self, BootSpec, Machine, fields, number},
};
pub use session::Session;
use std::{fs, path::Path};
pub fn bad(message: impl Into<String>) -> Error {
    Error::new("graphics-contract", message)
}
pub fn read_serial(path: &Path) -> Result<String> {
    if fs::metadata(path)?.len() > 2 * 1024 * 1024 {
        return Err(bad("serial exceeds 2 MiB"));
    }
    Ok(fs::read_to_string(path)?)
}
pub const CASES: &[&str] = &[
    "t07-resolution",
    "t07-surface-allocation",
    "t07-surface-description",
    "t07-ps2-timeout",
    "t07-corrupt-input",
    "t07-input-overflow",
    "t07-clipping",
    "t07-redraw",
];
pub fn execute(
    context: &Context,
    runner: &mut Runner,
    manifest: &Manifest,
    machine: &Machine,
    kernel: &Path,
) -> Result<()> {
    for mode in CASES {
        let disk = boot_image::create(&context.root, runner, manifest, kernel, mode)?;
        vm::boot(
            runner,
            manifest,
            machine,
            BootSpec {
                boot_disk: &disk,
                data_disk: None,
                readonly: true,
                mode,
            },
        )?;
        println!("graphics {mode}: verified");
    }
    Ok(())
}
fn one<'a>(output: &'a str, prefix: &str) -> Result<&'a str> {
    let mut lines = output.lines().filter(|l| l.starts_with(prefix));
    let value = lines
        .next()
        .ok_or_else(|| bad(format!("missing {prefix}")))?;
    if lines.next().is_some() {
        return Err(bad(format!("duplicate {prefix}")));
    }
    Ok(value)
}
pub fn validate(output: &str, mode: &str) -> Result<()> {
    let label = if mode.is_empty() { "normal" } else { mode };
    if one(output, "RR:T07:ENTER ")? != format!("RR:T07:ENTER case={label}") {
        return Err(bad("case identity"));
    }
    if mode.is_empty() {
        if output.contains("RR:T07:INJECT") {
            return Err(bad("injection in release"));
        }
    } else if one(output, "RR:T07:INJECT ")? != format!("RR:T07:INJECT case={mode}") {
        return Err(bad("injection identity"));
    }
    match mode {
        "t07-surface-allocation" | "t07-surface-description" => {
            let code = if mode == "t07-surface-allocation" {
                "SURFACE_ALLOCATION"
            } else {
                "SURFACE_PITCH"
            };
            if one(output, "RR:T07:FAIL ")? != format!("RR:T07:FAIL code={code} stage=initialize")
                || output.contains("RR:T07:READY")
                || output.contains("RR:T07:FRAME")
            {
                return Err(bad("surface failure falsely rendered/ready"));
            }
            let f = fields(one(output, "RR:T07:SURFACE_REJECT ")?)?;
            if number(&f, "writes")? != 0 || number(&f, "before")? != number(&f, "after")? {
                return Err(bad("surface failure wrote or leaked"));
            }
            return Ok(());
        }
        "t07-ps2-timeout" => {
            let f = fields(one(output, "RR:T07:PS2_FAIL ")?)?;
            if !matches!(
                f.get("code"),
                Some(&"DEADLINE") | Some(&"POLL_BUDGET_EXHAUSTED")
            ) || f.get("stage") != Some(&"keyboard-reset")
                || number(&f, "irq_masked")? != 1
                || output.contains("RR:T07:READY")
            {
                return Err(bad("PS2 timeout failure contract"));
            }
            let terminal = fields(one(output, "RR:T07:FAIL ")?)?;
            if terminal.get("code") != f.get("code") || terminal.get("stage") != Some(&"initialize")
            {
                return Err(bad("PS2 terminal differs"));
            }
            let frame = fields(one(output, "RR:T07:FRAME ")?)?;
            if frame.get("status") != Some(&"offline") {
                return Err(bad("PS2 failure not visible"));
            }
            return Ok(());
        }
        _ => {}
    }
    if output.contains("RR:T07:FAIL") || output.contains("RR:T07:OFFLINE") {
        return Err(bad("unexpected failure"));
    }
    let ready = fields(one(output, "RR:T07:READY ")?)?;
    if ready.get("case") != Some(&label) || number(&ready, "generation")? != 1 {
        return Err(bad("invalid READY"));
    }
    let ps2 = fields(one(output, "RR:T07:PS2 ")?)?;
    for (key, value) in [
        ("keyboard_set", 2),
        ("translation", 0),
        ("mouse_id", 0),
        ("packet_bytes", 3),
        ("scale", 1),
        ("irq_keyboard", 1),
        ("irq_mouse", 12),
    ] {
        if number(&ps2, key)? != value {
            return Err(bad("PS2 mode differs"));
        }
    }
    let surface = fields(one(output, "RR:T07:SURFACE ")?)?;
    let (width, height): (u64, u64) = if mode == "t07-resolution" {
        (800, 600)
    } else {
        (1024, 768)
    };
    let pixel_bytes = width * height * 4;
    let page_bytes = pixel_bytes.div_ceil(4096) * 4096 + 8192;
    if number(&surface, "width")? != width
        || number(&surface, "height")? != height
        || number(&surface, "pixels_bytes")? != pixel_bytes
        || number(&surface, "page_bytes")? != page_bytes
        || number(&surface, "allocated_after")?.checked_sub(number(&surface, "allocated_before")?)
            != Some(page_bytes)
    {
        return Err(bad("surface accounting/layout"));
    }
    let frames = output
        .lines()
        .filter(|l| l.starts_with("RR:T07:FRAME "))
        .map(fields)
        .collect::<Result<Vec<_>>>()?;
    if frames.is_empty() {
        return Err(bad("no rendered frame"));
    }
    let mut events = 0;
    let mut releases = 0;
    for (i, f) in frames.iter().enumerate() {
        if number(f, "generation")? != i as u64 + 1
            || number(f, "events")? < events
            || number(f, "releases")? < releases
            || number(f, "sentinel")? != 1
            || number(f, "x")? >= width
            || number(f, "y")? >= height
            || number(f, "frame_bytes")? != number(&surface, "allocated_after")?
            || number(f, "heap")? != number(&surface, "heap")?
        {
            return Err(bad("frame sequence/bounds/accounting"));
        }
        events = number(f, "events")?;
        releases = number(f, "releases")?;
    }
    let last = frames.last().unwrap();
    if matches!(mode, "t07-corrupt-input" | "t07-input-overflow") {
        let reset = fields(one(output, "RR:T07:RESET ")?)?;
        if number(&reset, "modifiers")? != 0
            || number(&reset, "left")? != 0
            || number(&reset, "count")? != 1
            || number(&reset, "text_bytes")? != 0
            || number(last, "resets")? != 1
            || last.get("text") != Some(&"61")
            || releases < 2
        {
            return Err(bad("reset/release/recovery evidence"));
        }
        if one(output, "RR:T07:RECOVERED ")? != "RR:T07:RECOVERED count=1 modifiers=0 left=0" {
            return Err(bad("recovery evidence"));
        }
    } else if number(last, "resets")? != 0
        || number(last, "raw_dropped")? != 0
        || number(last, "event_dropped")? != 0
    {
        return Err(bad("unexpected input loss"));
    }
    if mode == "t07-redraw"
        && (frames.len() < 101
            || releases < 100
            || number(last, "ticks")? <= number(&frames[0], "ticks")?)
    {
        return Err(bad("100 input-driven frames not proven"));
    }
    Ok(())
}

pub fn validate_recording(output: &str, mode: &str, full: bool, gui: bool) -> Result<()> {
    validate(output, mode)?;
    if full || gui {
        let last = output
            .lines()
            .rfind(|line| line.starts_with("RR:T07:FRAME "))
            .ok_or_else(|| bad("missing interaction frame"))?;
        let frame = fields(last)?;
        if number(&frame, "releases")? < 2
            || number(&frame, "button_releases")? < 1
            || number(&frame, "activate0")?.saturating_add(number(&frame, "activate1")?) < 1
            || number(&frame, "left")? != 0
        {
            return Err(bad("missing recorded release/button/activation evidence"));
        }
        if full
            && (frame.get("text") != Some(&"61")
                || frame.get("status") != Some(&"ready")
                || !output.lines().any(|line| {
                    line.starts_with("RR:T07:FRAME ") && line.contains(" status=full ")
                }))
        {
            return Err(bad("missing capacity and fresh-input evidence"));
        }
    }
    Ok(())
}

/// 对本次真实串口的坏副本执行同一验收入口，记录拒绝原因；不另启动 guest。
pub fn negative_controls(
    output: &str,
    mode: &str,
    full: bool,
    gui: bool,
    directory: &Path,
) -> Result<()> {
    let mut cases = Vec::new();
    if let Some(ready) = output
        .lines()
        .find(|line| line.starts_with("RR:T07:READY "))
    {
        cases.push(("duplicate-ready", format!("{output}\n{ready}\n")));
        cases.push((
            "false-ready",
            output.replace(ready, &ready.replace("generation=1", "generation=2")),
        ));
        let missing = output
            .lines()
            .map(|line| {
                if line.starts_with("RR:T07:FRAME ") {
                    line.split_whitespace()
                        .filter(|field| !field.starts_with("releases="))
                        .collect::<Vec<_>>()
                        .join(" ")
                } else {
                    line.into()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        cases.push(("missing-release", missing));
    } else {
        cases.push((
            "false-ready",
            format!("{output}\nRR:T07:READY case={mode} generation=1\n"),
        ));
    }
    if output.contains("RR:T07:RESET ") {
        cases.push((
            "missing-reset",
            output
                .lines()
                .filter(|line| !line.starts_with("RR:T07:RESET "))
                .collect::<Vec<_>>()
                .join("\n")
                + "\n",
        ));
    }
    let mut report = String::new();
    for (name, modified) in cases {
        let error = validate_recording(&modified, mode, full, gui)
            .err()
            .ok_or_else(|| bad(format!("accepted damaged recording: {name}")))?;
        fs::write(directory.join(format!("negative-{name}.log")), modified)?;
        report.push_str(&format!(
            "{name}: rejected kind={} reason={}\n",
            error.kind, error.detail
        ));
    }
    fs::write(directory.join("negative-controls.txt"), report)?;
    Ok(())
}
