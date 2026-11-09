//! AT07 工作台交互与独立像素检查。
use super::{Ppm, Session, Snapshot, bad, cases, fields, json_lines, number};
use crate::{
    boot_image::Manifest,
    error::Result,
    process::Runner,
    vm::{self, BootSpec, Machine},
    vm_fat,
};
use serde_json::json;
use std::path::Path;

pub(crate) const MODE: &str = "t10-at07";
const TERMINAL: &str = "at07-terminal-error";
const DETAIL: &str = "at07-operation-detail";
const FILES: &str = "at07-files-only";
const REFRESH: &str = "at07-keyboard-refresh";
const DUAL: &str = "at07-dual-restored";

pub(crate) fn execute(
    runner: &mut Runner,
    manifest: &Manifest,
    machine: &Machine,
    disk: &Path,
) -> Result<()> {
    let before = manifest.number("disk_bytes")?;
    if disk.metadata()?.len() != before {
        return Err(bad("AT07 source disk size"));
    }
    let before = vm_fat::sha(disk, runner)?;
    vm::boot(
        runner,
        manifest,
        machine,
        BootSpec {
            boot_disk: disk,
            data_disk: None,
            readonly: true,
            mode: MODE,
        },
    )?;
    if vm_fat::sha(disk, runner)? != before {
        return Err(bad("AT07 read-only disk changed"));
    }
    runner.record("at07_workbench=pass keyboard_pointer=verified host_pixels=exact independent_pixels=pass disk_sha256=unchanged")?;
    println!("AT07 workbench: layouts, focus, error text and read-only disk verified");
    Ok(())
}

impl Session {
    pub(super) fn at07_steps(&mut self) -> Result<()> {
        self.click(970, 16)?;
        self.wait(|s| {
            let f = fields(s)?;
            Ok(number(&f, "single")? == 1
                && number(&f, "terminal")? == 1
                && f.get("focus") == Some(&"Input"))
        })?;
        self.command("write denied.txt x", 1)?;
        self.snapshot(TERMINAL)?;

        for focus in [
            "PreviousResult",
            "NextResult",
            "OutputUp",
            "OutputDown",
            "Details",
        ] {
            self.at07_tab(focus)?;
        }
        self.tap("ret")?;
        self.wait(|s| {
            let f = fields(s)?;
            Ok(number(&f, "details")? == 1
                && number(&f, "pending")? == 0
                && f.get("focus") == Some(&"CloseDetails"))
        })?;
        self.snapshot(DETAIL)?;
        self.tap("ret")?;
        self.wait(|s| {
            let f = fields(s)?;
            Ok(number(&f, "details")? == 0 && f.get("focus") == Some(&"Input"))
        })?;

        self.click(970, 16)?;
        self.wait(|s| {
            let f = fields(s)?;
            Ok(number(&f, "single")? == 1
                && number(&f, "terminal")? == 0
                && f.get("focus") == Some(&"Files"))
        })?;
        self.snapshot(FILES)?;
        self.at07_tab("Refresh")?;
        self.tap("ret")?;
        self.wait(|s| {
            let f = fields(s)?;
            Ok(number(&f, "pending")? == 0 && f.get("focus") == Some(&"Refresh"))
        })?;
        self.snapshot(REFRESH)?;
        for focus in ["NextPage", "FilesUp", "FilesDown", "Panels"] {
            self.at07_tab(focus)?;
        }
        self.tap("ret")?;
        self.wait(|s| {
            let f = fields(s)?;
            Ok(number(&f, "single")? == 0 && f.get("focus") == Some(&"Files"))
        })?;
        self.snapshot(DUAL)
    }

    fn at07_tab(&mut self, focus: &str) -> Result<()> {
        self.tap("tab")?;
        let state = self.state()?;
        if fields(&state)?.get("focus") != Some(&focus) {
            return Err(bad(format!("AT07 Tab did not focus {focus}")));
        }
        Ok(())
    }
}

pub(super) fn validate(output: &str, snapshots: &[Snapshot]) -> Result<()> {
    cases::live_device(output, true)?;
    let surfaces: Vec<_> = output
        .lines()
        .filter(|s| s.starts_with("RR:T07:SURFACE "))
        .collect();
    let [surface] = surfaces.as_slice() else {
        return Err(bad("AT07 surface record count"));
    };
    let surface = fields(surface)?;
    for (field, expected) in [
        ("width", 1024),
        ("height", 768),
        ("pitch", 4096),
        ("pixels_bytes", 3_145_728),
        ("red", 16),
        ("green", 8),
        ("blue", 0),
    ] {
        if number(&surface, field)? != expected {
            return Err(bad(format!("AT07 surface {field}")));
        }
    }
    let replies = json_lines(output, "RR:T10:REPLY ")?;
    let details = json_lines(output, "RR:T10:DETAIL ")?;
    let ([reply], [detail]) = (replies.as_slice(), details.as_slice()) else {
        return Err(bad("AT07 error and detail count"));
    };
    let error = &reply["error"]["detail"];
    let record = &detail["output"]["record"];
    if reply["ok"] != false
        || reply["command_id"] != 1
        || reply["error"]["kind"] != "service"
        || !reply["operation_id"].is_string()
        || error["code"] != "ReadOnly"
        || error["stage"] != "Validate"
        || error["object"] != "/data/denied.txt"
        || error["partial_effect"]
            != json!({
                "created": false, "written_bytes": 0, "may_have_changed": false,
                "persistence_confirmed": false,
            })
        || detail["ok"] != true
        || detail["output"]["kind"] != "explanation"
        || record["operation_id"] != reply["operation_id"]
        || record["status"] != "Failed"
        || record["error"] != *error
    {
        return Err(bad("AT07 read-only error or original operation detail"));
    }
    let queries: Vec<_> = output
        .lines()
        .filter(|s| s.starts_with("RR:T11:PAGE "))
        .collect();
    if queries
        != [
            "RR:T11:PAGE query=2 generation=0 source=0 update=idle ok=1",
            "RR:T11:PAGE query=4 generation=0 source=0 update=idle ok=1",
        ]
    {
        return Err(bad("AT07 keyboard refresh query"));
    }
    let expected = [
        ("workbench-initial", 0, 1, 0, 0, 0, "Input"),
        (TERMINAL, 1, 1, 0, 1, 1, "Input"),
        (DETAIL, 1, 1, 1, 1, 1, "CloseDetails"),
        (FILES, 1, 0, 0, 1, 2, "Files"),
        (REFRESH, 1, 0, 0, 1, 2, "Refresh"),
        (DUAL, 0, 0, 0, 1, 2, "Files"),
    ];
    if snapshots.len() != expected.len() {
        return Err(bad("AT07 snapshot count"));
    }
    for (snapshot, (name, single, terminal, detail, results, clicks, focus)) in
        snapshots.iter().zip(expected)
    {
        let frame = fields(&snapshot.frame)?;
        if snapshot.name != name || frame.get("focus") != Some(&focus) {
            return Err(bad(format!("AT07 wrong snapshot or focus: {name}")));
        }
        for (field, value) in [
            ("single", single),
            ("terminal", terminal),
            ("details", detail),
            ("results", results),
            ("button_releases", clicks),
            ("pending", 0),
            ("left", 0),
            ("resets", 0),
        ] {
            if number(&frame, field)? != value {
                return Err(bad(format!("AT07 {name}: wrong {field}")));
            }
        }
    }
    Ok(())
}

const BACKGROUND: [u8; 3] = [239, 242, 239];
const BORDER: [u8; 3] = [176, 190, 181];
const FOCUS: [u8; 3] = [36, 99, 80];
const SURFACE: [u8; 3] = [252, 253, 251];
const TEXT: [u8; 3] = [32, 47, 42];

pub(super) fn pixels(image: &Ppm, name: &str) -> Result<()> {
    let samples: &[(usize, usize, [u8; 3])] = match name {
        "workbench-initial" => &[(8, 82, BORDER), (413, 82, FOCUS), (408, 82, BACKGROUND)],
        TERMINAL => &[(8, 82, FOCUS), (408, 82, SURFACE), (1015, 82, FOCUS)],
        DETAIL => &[(8, 480, FOCUS), (10, 478, BACKGROUND), (1007, 503, FOCUS)],
        FILES => &[(8, 82, FOCUS), (408, 82, SURFACE), (1015, 82, FOCUS)],
        REFRESH => &[(100, 129, FOCUS)],
        DUAL => &[(8, 82, FOCUS), (413, 82, BORDER), (408, 82, BACKGROUND)],
        _ => return Err(bad("AT07 unknown screenshot")),
    };
    for &(x, y, expected) in samples {
        if image.pixel(x, y)? != expected {
            return Err(bad(format!("AT07 {name}: pixel at {x},{y}")));
        }
    }
    if name == "workbench-initial" || name == DUAL {
        for y in 80..732 {
            if image.pixel(408, y)? != BACKGROUND {
                return Err(bad("AT07 content crossed the panel gutter"));
            }
        }
    }
    if name == TERMINAL || name == DUAL {
        // BDF 10x20 的 F / R 第四行，分别定位到 FAILED 与 ReadOnly 的开头。
        let origin = if name == TERMINAL { 16 } else { 421 };
        for (x, mask) in [(origin, 0x7f80_u16), (origin + 70, 0x7e00_u16)] {
            for bit in 0..10 {
                let color = if mask & (0x8000 >> bit) == 0 {
                    SURFACE
                } else {
                    TEXT
                };
                if image.pixel(x + bit, 203)? != color {
                    return Err(bad(format!("AT07 {name}: error text glyph")));
                }
            }
        }
    }
    Ok(())
}
