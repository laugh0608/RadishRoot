//! T14 的 AT08 子项。普通内核、真实 PS/2 输入、正常关机及独立 FAT 校验。
use super::{Ppm, Session, Snapshot, bad, cases, fields, json_lines, number, power};
use crate::{
    boot_image::Manifest,
    config::Context,
    error::Result,
    process::Runner,
    vm::{self, Machine},
    vm_fat::{self, Expected, FatCheck},
    vm_storage,
};
use serde_json::Value;
use std::path::Path;

const WRITE: &str = "t14-write";
const READONLY: &str = "t14-readonly";
const CANCEL: &str = "t14-cancel-focused";
const CONFIRM: &str = "t14-delete-focused";
const DRAFT: &str = "write escaped.txt x";

pub(crate) fn mode(readonly: bool) -> &'static str {
    if readonly { READONLY } else { WRITE }
}

pub(super) fn is_directory_case(mode: &str) -> bool {
    matches!(mode, WRITE | READONLY)
}

pub(crate) fn execute(
    context: &Context,
    runner: &mut Runner,
    manifest: &Manifest,
    machine: &Machine,
    source: &Path,
) -> Result<()> {
    let data = vm_storage::clone_sparse(source, runner)?;
    let check = |runner: &mut Runner, mode, complete| {
        let mut files = cases::fixture(false, false);
        if complete {
            files.push(Expected {
                id: "keep",
                path: "work/keep.txt".into(),
                bytes: b"keep".to_vec(),
            });
        }
        vm_fat::tools_with_files(
            context,
            runner,
            manifest,
            &data,
            FatCheck {
                mode,
                files,
                directories: if complete { vec!["work"] } else { vec![] },
            },
        )
    };
    check(runner, "prepare", false)?;
    check(runner, "verify", false)?;
    vm::boot_directories(runner, manifest, machine, source, &data, false)?;
    check(runner, "verify", true)?;
    vm_fat::outside_unchanged(source, &data, manifest)?;
    let saved = vm_fat::sha(&data, runner)?;
    vm::boot_directories(runner, manifest, machine, source, &data, true)?;
    if vm_fat::sha(&data, runner)? != saved {
        return Err(bad("T14 read-only reopen changed disk"));
    }
    runner.record(&format!("t14_at08=pass data={} boots=2 normal_shutdowns=2 independent_fat=exact outside_data=unchanged readonly_disk=unchanged", data.display()))?;
    println!(
        "T14 directory workflow verified: input isolation, two guest shutdowns, exact independent FAT and read-only reopen"
    );
    Ok(())
}

impl Session {
    pub(super) fn directory_poll(&mut self, state: &str) -> Result<bool> {
        if fields(state)?.get("power") == Some(&"failed") {
            return Err(bad("T14 normal shutdown blocked"));
        }
        if self.power_done {
            return Ok(false);
        }
        let shutdown = if self.mode == WRITE {
            self.t14_write()?;
            11
        } else {
            self.t14_reopen()?;
            8
        };
        self.command("shutdown", shutdown)?;
        self.power_done = true;
        Ok(false)
    }

    fn t14_focus(&self, focus: &str) -> Result<()> {
        self.wait(|s| Ok(fields(s)?.get("focus") == Some(&focus)))?;
        Ok(())
    }

    fn t14_page(&self, entries: u64) -> Result<()> {
        self.wait(|s| {
            let frame = fields(s)?;
            Ok(number(&frame, "pending")? == 0
                && number(&frame, "list_ok")? == 1
                && number(&frame, "entries")? == entries)
        })?;
        Ok(())
    }

    fn t14_details(&mut self, name: &str) -> Result<()> {
        self.click(27, 687)?;
        self.t14_focus("CloseDetails")?;
        self.snapshot(name)?;
        self.tap("esc")
    }

    fn t14_write(&mut self) -> Result<()> {
        self.click(40, 138)?;
        self.t14_focus("DialogInput")?;
        self.type_text("work")?;
        self.tap("ret")?;
        self.wait_for_visible("work", 1)?;
        self.t14_details("t14-created-details")?;

        self.click(50, 91)?;
        self.t14_focus("FilePath")?;
        self.type_text("/work")?;
        self.tap("ret")?;
        self.t14_page(0)?;
        self.snapshot("t14-path-navigation")?;
        self.click(450, 688)?;
        self.command("pwd", 1)?;
        self.click(330, 714)?;
        self.t14_focus("Input")?;
        self.command("pwd", 2)?;
        self.command("mkdir empty", 3)?;
        self.wait_for_visible("empty", 2)?;
        self.command("write keep.txt keep", 4)?;
        self.wait_for_visible("keep.txt", 3)?;
        self.command("write drop.txt gone", 5)?;
        self.wait_for_visible("drop.txt", 4)?;
        self.command("rm /data/work", 6)?;
        self.command("rm ../../escape", 7)?;
        self.command("rm /data", 8)?;

        // 保留文件作为取消的独立读盘对照，终端留有可检测的背景草稿。
        self.type_text(DRAFT)?;
        self.click(36, 713)?;
        self.wait(|s| Ok(number(&fields(s)?, "file_row")? == 1))?;
        self.click(150, 139)?;
        self.t14_focus("DialogCancel")?;
        self.click(450, 688)?;
        self.type_text("x")?;
        self.t14_focus("DialogCancel")?;
        self.snapshot(CANCEL)?;
        self.tap("ret")?;
        self.wait(|s| {
            let f = fields(s)?;
            Ok(f.get("focus") == Some(&"Files")
                && number(&f, "results")? == 8
                && number(&f, "input_bytes")? == DRAFT.len() as u64
                && number(&f, "list_generation")? == 4
                && number(&f, "entries")? == 3
                && number(&f, "pending")? == 0)
        })?;
        self.tap("esc")?;

        self.click(121, 713)?;
        self.wait(|s| Ok(number(&fields(s)?, "file_row")? == 2))?;
        self.click(150, 139)?;
        self.t14_focus("DialogCancel")?;
        self.tap("tab")?;
        self.t14_focus("DialogConfirm")?;
        self.snapshot(CONFIRM)?;
        self.tap("ret")?;
        self.t14_page(2)?;
        self.t14_details("t14-removed-details")?;

        self.command("rm empty", 9)?;
        self.t14_page(1)?;
        self.wait_for_visible("keep.txt", 6)?;
        self.click(208, 115)?;
        self.t14_page(2)?;
        self.snapshot("t14-parent-navigation")?;
        self.click(450, 688)?;
        self.command("pwd", 10)
    }

    fn t14_reopen(&mut self) -> Result<()> {
        self.command("cd work", 1)?;
        self.t14_page(2)?;
        self.command("cd keep.txt", 2)?;
        self.command("cd /data", 3)?;
        self.click(39, 183)?;
        self.t14_focus("Files")?;
        self.tap("ret")?;
        self.t14_page(1)?;
        self.click(330, 714)?;
        self.t14_focus("Input")?;
        self.command("pwd", 4)?;
        self.command("cat keep.txt", 5)?;
        self.command("mkdir denied", 6)?;
        self.command("rm keep.txt", 7)?;
        self.snapshot("t14-readonly-refusal")
    }
}

fn directory(reply: &Value, path: &str) -> bool {
    reply["ok"] == true && reply["output"]["kind"] == "directory" && reply["output"]["path"] == path
}

fn rejected(reply: &Value, code: &str, stage: &str) -> bool {
    let error = &reply["error"]["detail"];
    reply["ok"] == false
        && reply["error"]["kind"] == "service"
        && error["code"] == code
        && error["stage"] == stage
        && error["partial_effect"]["may_have_changed"] == false
        && error["partial_effect"]["persistence_confirmed"] == false
}

pub(super) fn validate(
    output: &str,
    mode: &str,
    boot: [u8; 16],
    snapshots: &[Snapshot],
) -> Result<()> {
    let readonly = mode == READONLY;
    cases::live_device(output, readonly)?;
    let boot = format!("{:032x}:", u128::from_be_bytes(boot));
    let replies = json_lines(output, "RR:T10:REPLY ")?;
    let details = json_lines(output, "RR:T10:DETAIL ")?;
    if replies
        .iter()
        .chain(&details)
        .filter_map(|r| r["operation_id"].as_str())
        .any(|id| !id.starts_with(&boot))
    {
        return Err(bad("T14 operation boot identity"));
    }
    let shutdown = if readonly {
        let [
            enter,
            wrong_kind,
            return_to_root,
            pwd,
            cat,
            mkdir,
            rm,
            shutdown,
        ] = replies.as_slice()
        else {
            return Err(bad("T14 read-only reply count"));
        };
        if !directory(enter, "/data/work")
            || !rejected(wrong_kind, "WrongKind", "List")
            || !directory(return_to_root, "/data")
            || !directory(pwd, "/data/work")
            || cat["ok"] != true
            || cat["output"]["kind"] != "bytes"
            || cat["output"]["hex"] != "6b656570"
            || !rejected(mkdir, "ReadOnly", "Validate")
            || !rejected(rm, "ReadOnly", "Validate")
            || !details.is_empty()
        {
            return Err(bad("T14 read-only navigation, persistence, or refusal"));
        }
        shutdown
    } else {
        let [
            before,
            after,
            mkdir,
            keep,
            drop,
            nonempty,
            outside,
            root,
            rm,
            parent,
            shutdown,
        ] = replies.as_slice()
        else {
            return Err(bad("T14 writable reply count"));
        };
        if !directory(before, "/data")
            || !directory(after, "/data/work")
            || !directory(parent, "/data/work")
            || [
                (mkdir, "/data/work/empty"),
                (keep, "/data/work/keep.txt"),
                (drop, "/data/work/drop.txt"),
            ]
            .iter()
            .any(|(r, p)| {
                r["ok"] != true
                    || r["output"]["kind"] != "created"
                    || r["output"]["reference"]["path"] != *p
            })
            || !rejected(nonempty, "DirectoryNotEmpty", "Remove")
            || !rejected(outside, "InvalidPath", "Validate")
            || !rejected(root, "RootDirectory", "Validate")
            || rm["ok"] != true
            || rm["output"]["kind"] != "removed"
            || rm["output"]["path"] != "/data/work/empty"
        {
            return Err(bad("T14 writable command contract"));
        }
        let [created, removed] = details.as_slice() else {
            return Err(bad("T14 file action detail count"));
        };
        for (detail, kind, path, frame_name) in [
            (
                created,
                "CreateDirectory",
                "/data/work",
                "t14-created-details",
            ),
            (
                removed,
                "Remove",
                "/data/work/drop.txt",
                "t14-removed-details",
            ),
        ] {
            let snapshot = snapshots
                .iter()
                .find(|s| s.name == frame_name)
                .ok_or_else(|| bad("missing T14 action detail frame"))?;
            let source = number(&fields(&snapshot.frame)?, "change")?;
            let record = &detail["output"]["record"];
            if detail["ok"] != true
                || source == 0
                || record["operation_id"] != format!("{boot}{source}")
                || record["request_kind"] != kind
                || record["target"] != path
                || record["status"] != "Succeeded"
                || record["partial_effect"]["persistence_confirmed"] != true
            {
                return Err(bad("T14 file action identity or writeback"));
            }
        }
        for (name, focus) in [(CANCEL, "DialogCancel"), (CONFIRM, "DialogConfirm")] {
            let frame = snapshots
                .iter()
                .find(|s| s.name == name)
                .ok_or_else(|| bad("missing T14 modal frame"))?;
            let f = fields(&frame.frame)?;
            let input_bytes = if name == CANCEL {
                DRAFT.len() as u64
            } else {
                0
            };
            if f.get("focus") != Some(&focus)
                || number(&f, "input_bytes")? != input_bytes
                || number(&f, "results")? != 8
                || number(&f, "resets")? != 0
                || number(&f, "list_generation")? != 4
            {
                return Err(bad("T14 modal input isolation or focus"));
            }
        }
        shutdown
    };
    power::validate_request(output, shutdown, "Shutdown")
}

pub(super) fn pixels(image: &Ppm, name: &str) -> Result<()> {
    let selected = match name {
        CANCEL => 615,
        CONFIRM => 735,
        _ => return Ok(()),
    };
    let unselected = if selected == 615 { 735 } else { 615 };
    if image.pixel(272, 308)? != [36, 99, 80]
        || image.pixel(selected, 450)? != [36, 99, 80]
        || image.pixel(unselected, 450)? != [239, 242, 239]
    {
        return Err(bad(format!("T14 modal focus pixels: {name}")));
    }
    Ok(())
}
