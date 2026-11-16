//! T15 的 AT08 / AT12 定向验收，分别保留正常关机和覆盖故障的盘面。
use super::{Ppm, Session, Snapshot, bad, cases, fields, json_lines, number, power};
use crate::{
    boot_image::{self, Manifest},
    build,
    config::Context,
    error::Result,
    process::Runner,
    vm::{self, BootSpec, Machine},
    vm_fat::{self, Expected, FatCheck},
    vm_storage,
};
use serde_json::Value;
use std::{fs, path::Path};

const WRITE: &str = "t15-transfers";
const READONLY: &str = "t15-readonly";
pub(crate) const FAILURE: &str = "t15-copy-fail";
const CANCEL: &str = "t15-overwrite-cancel";
const CONFIRM: &str = "t15-overwrite-confirm";
const DRAFT: &str = "write escaped.txt x";
const LENGTH: usize = 9001;

pub(super) fn normal_shutdown(mode: &str) -> bool {
    matches!(mode, WRITE | READONLY)
}

pub(super) fn is_transfer_case(mode: &str) -> bool {
    normal_shutdown(mode) || mode == FAILURE
}

fn fixture(complete: bool, fault_observation: bool) -> Vec<Expected> {
    let payload: Vec<u8> = (0..LENGTH).map(|at| ((at * 17 + 91) % 256) as u8).collect();
    let mut files = vec![];
    let mut add = |id, path: &str, bytes: &[u8]| {
        files.push(Expected {
            id,
            path: path.into(),
            bytes: bytes.to_vec(),
        })
    };
    // 保留写入次序，独立工具注入的第一行是图形操作源。
    add("input", "input.bin", &payload);
    add("keep", "keep.bin", b"cancel must preserve these bytes\n");
    if !fault_observation {
        add(
            "replace",
            "replace.bin",
            if complete {
                &payload
            } else {
                b"previous GUI target\n"
            },
        );
    }
    add(
        "terminal",
        "terminal.bin",
        if complete {
            &payload
        } else {
            b"previous terminal target\n"
        },
    );
    add(
        "text",
        "text.txt",
        if complete {
            b"short"
        } else {
            b"old text with trailing bytes\n"
        },
    );
    if complete {
        add("moved", "out/moved.bin", &payload);
        add("graph", "out/graphmoved.bin", &payload);
    }
    files
}

pub(crate) fn execute(
    context: &Context,
    runner: &mut Runner,
    manifest: &Manifest,
    machine: &Machine,
    source: &Path,
) -> Result<()> {
    let verify = |runner: &mut Runner, disk: &Path, mode, complete, fault_observation| {
        vm_fat::tools_with_files(
            context,
            runner,
            manifest,
            disk,
            FatCheck {
                mode,
                files: fixture(complete, fault_observation),
                directories: if complete { vec!["out"] } else { vec![] },
            },
        )
    };
    let disk = vm_storage::clone_sparse(source, runner)?;
    verify(runner, &disk, "prepare", false, false)?;
    verify(runner, &disk, "verify", false, false)?;
    vm::boot_workbench(runner, manifest, machine, source, &disk, false, WRITE)?;
    verify(runner, &disk, "verify", true, false)?;
    vm_fat::outside_unchanged(source, &disk, manifest)?;
    let saved = vm_fat::sha(&disk, runner)?;
    vm::boot_workbench(runner, manifest, machine, source, &disk, true, READONLY)?;
    if vm_fat::sha(&disk, runner)? != saved {
        return Err(bad("T15 read-only reopen changed DATA disk"));
    }
    runner.record(&format!("t15_normal=pass data={} natural_shutdowns=2 exact_fat=pass outside_data=unchanged readonly_sha={saved}", disk.display()))?;

    let kernel = build::kernel(context, runner, true, true, true)?;
    let boot = boot_image::create(&context.root, runner, manifest, &kernel, FAILURE)?;
    let fault = vm_storage::clone_sparse(source, runner)?;
    verify(runner, &fault, "prepare", false, false)?;
    verify(runner, &fault, "verify", false, false)?;
    vm::boot(
        runner,
        manifest,
        machine,
        BootSpec {
            boot_disk: &boot,
            data_disk: Some(&fault),
            readonly: false,
            mode: FAILURE,
        },
    )?;
    let observation = verify(runner, &fault, "observe-copy", false, true)?;
    vm_fat::outside_unchanged(source, &fault, manifest)?;
    let read_exit = fs::read_to_string(observation.join("observe-copy-exit.txt"))?;
    let fsck_exit = fs::read_to_string(observation.join("fsck-exit.txt"))?;
    let observed = observation.join("observed-copy.bin");
    if read_exit.trim() == "0" {
        crate::digest(&observed, runner)?;
        runner.record(&format!(
            "t15_observed_target_bytes={}",
            observed.metadata()?.len()
        ))?;
    }
    runner.record(&format!("t15_failure=pass data={} observed={} target_read_exit={} fsck_exit={} outside_data=unchanged host_stopped=true", fault.display(), observation.display(), read_exit.trim(), fsck_exit.trim()))?;
    println!(
        "T15 transfers: normal and read-only guest shutdown, independent FAT, and interrupted overwrite verified"
    );
    Ok(())
}

impl Session {
    pub(super) fn transfer_poll(&mut self, frame: &str) -> Result<bool> {
        if self.mode == FAILURE {
            self.command("cp --overwrite input.bin replace.bin", 1)?;
            self.click(590, 116)?;
            self.wait_for_focus("CloseDetails")?;
            self.snapshot("t15-copy-failed-details")?;
            self.tap("esc")?;
            self.command("shutdown", 2)?;
            self.snapshot("t15-shutdown-blocked")?;
            return Ok(true);
        }
        if fields(frame)?.get("power") == Some(&"failed") {
            return Err(bad("T15 normal shutdown blocked"));
        }
        if self.power_done {
            return Ok(false);
        }
        let shutdown = if self.mode == WRITE {
            self.transfer_write()?;
            10
        } else {
            self.transfer_reopen()?;
            6
        };
        self.command("shutdown", shutdown)?;
        self.power_done = true;
        Ok(false)
    }

    fn file_row(&self, row: u64) -> Result<()> {
        self.wait(|state| Ok(number(&fields(state)?, "file_row")? == row))?;
        Ok(())
    }

    fn copy_input_to(&mut self, path: &str) -> Result<()> {
        let state = self.state()?;
        let row = number(&fields(&state)?, "file_row")?;
        for next in (0..row).rev() {
            self.click(36, 713)?;
            self.file_row(next)?;
        }
        self.click(291, 139)?;
        self.wait_for_focus("DialogInput")?;
        self.type_text(path)?;
        self.tap("ret")
    }

    fn transfer_write(&mut self) -> Result<()> {
        for (line, results) in [
            ("cp input.bin shell.bin", 1),
            ("cp input.bin keep.bin", 2),
            ("cp --overwrite input.bin INPUT.BIN", 3),
            ("cp input.bin ../outside.bin", 4),
            ("cp missing.bin never.bin", 5),
            ("write --overwrite text.txt short", 6),
            ("cp --overwrite input.bin terminal.bin", 7),
            ("mkdir out", 8),
            ("mv shell.bin out/moved.bin", 9),
        ] {
            self.command(line, results)?;
        }
        self.wait_for_file_count(6)?;
        self.copy_input_to("graph.bin")?;
        self.wait_for_visible("graph.bin", 6)?;
        self.snapshot_file_details("t15-copy-details")?;
        self.click(226, 139)?;
        self.wait_for_focus("DialogInput")?;
        self.type_text("out/graphmoved.bin")?;
        self.tap("ret")?;
        self.wait_for_file_count(6)?;
        self.snapshot_file_details("t15-rename-details")?;

        self.type_text(DRAFT)?;
        self.copy_input_to("keep.bin")?;
        self.wait_for_focus("DialogCancel")?;
        self.click(450, 688)?;
        self.type_text("x")?;
        self.snapshot(CANCEL)?;
        self.tap("ret")?;
        self.wait(|state| {
            let f = fields(state)?;
            Ok(f.get("focus") == Some(&"Files")
                && number(&f, "results")? == 9
                && number(&f, "input_bytes")? == DRAFT.len() as u64
                && number(&f, "list_generation")? == 7
                && number(&f, "pending")? == 0)
        })?;
        self.tap("esc")?;

        self.copy_input_to("replace.bin")?;
        self.wait_for_focus("DialogCancel")?;
        self.tap("tab")?;
        self.wait_for_focus("DialogConfirm")?;
        self.snapshot(CONFIRM)?;
        self.tap("ret")?;
        self.wait_for_visible("replace.bin", 8)?;
        self.snapshot_file_details("t15-overwrite-details")
    }

    fn transfer_reopen(&mut self) -> Result<()> {
        self.click(50, 91)?;
        self.wait_for_focus("FilePath")?;
        self.type_text("/out")?;
        self.tap("ret")?;
        self.wait_for_file_count(2)?;
        self.click(450, 688)?;
        for (line, results) in [
            ("ls out", 1),
            ("cat text.txt", 2),
            ("cp input.bin denied.bin", 3),
            ("cp --overwrite input.bin replace.bin", 4),
            ("write --overwrite text.txt no", 5),
        ] {
            self.command(line, results)?;
        }
        self.snapshot("t15-readonly-refusal")
    }
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

fn copied(reply: &Value, path: &str, overwritten: bool) -> bool {
    reply["ok"] == true
        && reply["output"]["kind"] == "copied"
        && reply["output"]["source"] == "/data/input.bin"
        && reply["output"]["reference"]["path"] == path
        && reply["output"]["bytes"] == LENGTH
        && reply["output"]["overwritten"] == overwritten
}

pub(super) fn validate(
    output: &str,
    mode: &str,
    boot: [u8; 16],
    snapshots: &[Snapshot],
) -> Result<()> {
    cases::live_device(output, mode == READONLY)?;
    let replies = json_lines(output, "RR:T10:REPLY ")?;
    let details = json_lines(output, "RR:T10:DETAIL ")?;
    let boot = format!("{:032x}:", u128::from_be_bytes(boot));
    if replies
        .iter()
        .filter_map(|r| r["operation_id"].as_str())
        .any(|id| !id.starts_with(&boot))
    {
        return Err(bad("T15 operation boot identity"));
    }
    let shutdown = match mode {
        WRITE => validate_rw(&replies, &details, snapshots, &boot)?,
        READONLY => validate_ro(&replies, &details)?,
        FAILURE => return validate_failure(output, &replies, &details),
        _ => unreachable!("transfer workbench case"),
    };
    power::validate_request(output, shutdown, "Shutdown")
}

fn validate_rw<'a>(
    replies: &'a [Value],
    details: &[Value],
    snapshots: &[Snapshot],
    boot: &str,
) -> Result<&'a Value> {
    let [
        create,
        conflict,
        alias,
        outside,
        missing,
        write,
        replace,
        mkdir,
        rename,
        shutdown,
    ] = replies
    else {
        return Err(bad("T15 writable reply count"));
    };
    if !copied(create, "/data/shell.bin", false)
        || !rejected(conflict, "AlreadyExists", "Create")
        || !rejected(alias, "SameFile", "CheckTarget")
        || !rejected(outside, "InvalidPath", "Validate")
        || !rejected(missing, "NotFound", "Read")
        || write["ok"] != true
        || write["output"]["kind"] != "overwritten"
        || write["output"]["reference"]["path"] != "/data/text.txt"
        || !copied(replace, "/data/terminal.bin", true)
        || mkdir["ok"] != true
        || mkdir["output"]["reference"]["path"] != "/data/out"
        || rename["ok"] != true
        || rename["output"]["kind"] != "renamed"
        || rename["output"]["source"] != "/data/shell.bin"
        || rename["output"]["reference"]["path"] != "/data/out/moved.bin"
    {
        return Err(bad("T15 writable command contract"));
    }
    let [copy, rename, replace] = details else {
        return Err(bad("T15 file action detail count"));
    };
    for (detail, name, kind, source, target, copied_bytes, truncated) in [
        (
            copy,
            "t15-copy-details",
            "Copy",
            "/data/input.bin",
            "/data/graph.bin",
            LENGTH,
            false,
        ),
        (
            rename,
            "t15-rename-details",
            "Rename",
            "/data/graph.bin",
            "/data/out/graphmoved.bin",
            0,
            false,
        ),
        (
            replace,
            "t15-overwrite-details",
            "Copy",
            "/data/input.bin",
            "/data/replace.bin",
            LENGTH,
            true,
        ),
    ] {
        let snapshot = snapshots
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| bad("missing transfer details frame"))?;
        let operation = number(&fields(&snapshot.frame)?, "change")?;
        let record = &detail["output"]["record"];
        let effect = &record["partial_effect"];
        if detail["ok"] != true
            || operation == 0
            || record["operation_id"] != format!("{boot}{operation}")
            || record["request_kind"] != kind
            || record["source"] != source
            || record["target"] != target
            || record["status"] != "Succeeded"
            || effect["written_bytes"] != copied_bytes
            || (effect["target_truncated"] == true) != truncated
            || effect["persistence_confirmed"] != true
            || (kind == "Copy" && record["total_bytes"] != LENGTH)
        {
            return Err(bad(format!("T15 original transfer detail: {name}")));
        }
    }
    for (name, focus, draft_bytes) in [
        (CANCEL, "DialogCancel", DRAFT.len()),
        (CONFIRM, "DialogConfirm", 0),
    ] {
        let snapshot = snapshots
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| bad("missing overwrite confirmation frame"))?;
        let f = fields(&snapshot.frame)?;
        if f.get("focus") != Some(&focus)
            || number(&f, "results")? != 9
            || number(&f, "input_bytes")? != draft_bytes as u64
            || number(&f, "resets")? != 0
            || number(&f, "list_generation")? != 7
        {
            return Err(bad("T15 modal input or draft isolation"));
        }
    }
    Ok(shutdown)
}

fn validate_ro<'a>(replies: &'a [Value], details: &[Value]) -> Result<&'a Value> {
    let [list, cat, copy, overwrite, write, shutdown] = replies else {
        return Err(bad("T15 read-only reply count"));
    };
    let listed = list["output"]["entries"].as_array();
    if list["ok"] != true
        || list["output"]["kind"] != "files"
        || !listed.is_some_and(|entries| {
            entries.len() == 2
                && ["moved.bin", "graphmoved.bin"]
                    .iter()
                    .all(|name| entries.iter().any(|entry| entry["name"] == *name))
        })
        || cat["ok"] != true
        || cat["output"]["hex"] != "73686f7274"
        || [copy, overwrite, write]
            .iter()
            .any(|reply| !rejected(reply, "ReadOnly", "Validate"))
        || !details.is_empty()
    {
        return Err(bad("T15 read-only directory, bytes or refusals"));
    }
    Ok(shutdown)
}

fn validate_failure(output: &str, replies: &[Value], details: &[Value]) -> Result<()> {
    let ([failed, shutdown], [detail]) = (replies, details) else {
        return Err(bad("T15 failure reply count"));
    };
    let error = &failed["error"]["detail"];
    let effect = &error["partial_effect"];
    let record = &detail["output"]["record"];
    if output
        .matches("RR:T15:INJECTION stage=Copy after_first_chunk=1 mechanism=adapter-once")
        .count()
        != 1
        || failed["ok"] != false
        || error["code"] != "Io"
        || error["stage"] != "Copy"
        || !error["cause"]
            .as_str()
            .is_some_and(|cause| cause.contains("phase: Write") && cause.contains("InjectedWrite"))
        || effect["written_bytes"] != 4096
        || effect["target_truncated"] != true
        || effect["created"] != false
        || effect["may_have_changed"] != true
        || effect["persistence_confirmed"] != false
        || detail["ok"] != true
        || record["operation_id"] != failed["operation_id"]
        || record["total_bytes"] != LENGTH
        || record["source"] != "/data/input.bin"
        || record["target"] != "/data/replace.bin"
        || record["status"] != "Failed"
        || record["error"] != *error
        || !rejected(shutdown, "Offline", "Validate")
        || output.contains("RR:T12:REQUEST ")
    {
        return Err(bad("T15 interrupted overwrite or shutdown refusal"));
    }
    Ok(())
}

pub(super) fn pixels(image: &Ppm, name: &str) -> Result<()> {
    let (selected, other) = match name {
        CANCEL => (615, 735),
        CONFIRM => (735, 615),
        _ => return Ok(()),
    };
    if image.pixel(272, 308)? != [36, 99, 80]
        || image.pixel(selected, 450)? != [36, 99, 80]
        || image.pixel(other, 450)? != [239, 242, 239]
    {
        return Err(bad(format!("T15 overwrite focus pixels: {name}")));
    }
    Ok(())
}
