//! AT06 独立期望与宿主 / UART guest 命令轨迹比较。
use super::validate::{self, one, validate_default};
use crate::{
    boot_image::Manifest,
    config::Context,
    error::{Error, Result},
    process::Runner,
    vm::{self, BootSpec, Machine},
    vm_fat::{self, Expected, FatCheck},
    vm_storage,
};
use serde_json::{Value, json};
use std::{fs, path::Path};

pub(crate) const MODE: &str = "t09-at06";
const OPERATIONS: [u64; 8] = [1, 0, 0, 2, 3, 4, 5, 0];
const HOST_BOOT: [u8; 16] = [1; 16];
const NOTE: &str = "at06 note.txt";
const PATH: &str = "/data/at06 note.txt";
const CREATE: &str = "write \"at06 note.txt\" ";
const BODY_LEN: usize = 1024 - CREATE.len();
const OVERFLOW: &str = "write overflow.txt ";

pub(super) fn input() -> Vec<u8> {
    format!(
        "ls /data\rwrite bad\runknown-at06\r{CREATE}{}\r{CREATE}again\r{OVERFLOW}{}\rcat \"{NOTE}\"\rls /data\roperations\r",
        "x".repeat(BODY_LEN),
        "y".repeat(1025 - OVERFLOW.len()),
    )
    .into_bytes()
}

fn need(ok: bool, field: &str) -> Result<()> {
    if !ok {
        return Err(Error::new("at06-trace", field));
    }
    Ok(())
}

#[derive(PartialEq)]
pub(super) enum Event {
    Reply(Value),
    Capacity,
}

pub(super) fn trace(s: &str, id: [u8; 16]) -> Result<Vec<Event>> {
    validate_default(s, id)?;
    need(
        one(s, "RR:T09:RESULT ")?
            == "RR:T09:RESULT commands=8 records=5 generation=1 changes=1 state=Ready",
        "service summary",
    )?;
    let boot = format!("{:032x}", u128::from_be_bytes(id));
    let volume = one(s, "RR:AT06:VOLUME ")?
        .strip_prefix("RR:AT06:VOLUME ")
        .unwrap();
    need(
        volume.len() == 32
            && u128::from_str_radix(volume, 16)
                .is_ok_and(|n| n != 0 && format!("{n:032x}") == volume),
        "volume identity",
    )?;
    let mut events = Vec::new();
    for line in s.lines() {
        if let Some(reply) = line.strip_prefix("RR:CONSOLE:REPLY ") {
            let reply = serde_json::from_str(reply)
                .map_err(|e| Error::new("at06-trace", format!("reply JSON: {e}")))?;
            events.push(Event::Reply(reply));
        } else if let Some(error) = line.strip_prefix("RR:CONSOLE:INPUT_ERROR ") {
            need(error == "code=Capacity line_discarded=1", "input rejection")?;
            events.push(Event::Capacity);
        }
    }
    need(
        events.len() == 10
            && events[5..7] == [Event::Capacity, Event::Capacity]
            && events.iter().filter(|e| **e == Event::Capacity).count() == 2,
        "ordered replies and complete overflow rejection",
    )?;
    let replies: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::Reply(reply) => Some(reply),
            Event::Capacity => None,
        })
        .collect();
    for (i, (reply, op)) in replies.iter().zip(OPERATIONS).enumerate() {
        need(
            reply["schema"] == "radishroot.command.v1"
                && reply["command_id"] == i + 1
                && reply["operation_id"]
                    == if op == 0 {
                        Value::Null
                    } else {
                        json!(format!("{boot}:{op}"))
                    }
                && reply["ok"] == [0, 3, 5, 6, 7].contains(&i),
            "reply identity or outcome",
        )?;
    }
    need(
        replies[0]["output"] == json!({"kind": "files", "more": false, "entries": []}),
        "initial empty listing",
    )?;
    for (reply, code) in [(replies[1], "Arguments"), (replies[2], "UnknownCommand")] {
        need(
            reply["error"] == json!({"kind": "parse", "code": code}),
            "parse error without an operation",
        )?;
    }
    let reference = json!({"boot_id": boot, "volume": volume, "generation": 1, "path": PATH});
    need(
        replies[3]["output"] == json!({"kind": "created", "reference": reference}),
        "1024-byte line created the expected file",
    )?;
    validate::error(replies[4], "AlreadyExists", "Create")?;
    validate::bytes(replies[5], &vec![b'x'; BODY_LEN])?;
    need(
        replies[6]["output"]
            == json!({"kind": "files", "more": false, "entries": [{
                "name": NOTE, "kind": "File", "size_bytes": BODY_LEN, "reference": reference,
            }]}),
        "only the original file exists after rejection",
    )?;
    let output = &replies[7]["output"];
    let records = output["records"]
        .as_array()
        .ok_or_else(|| Error::new("at06-trace", "operation records"))?;
    need(
        output["kind"] == "operations" && output["evicted"] == 0 && records.len() == 5,
        "operation snapshot",
    )?;
    for (i, record) in records.iter().enumerate() {
        need(
            record["operation_id"] == format!("{boot}:{}", i + 1)
                && record["request_kind"]
                    == ["List", "CreateText", "CreateText", "Read", "List"][i]
                && record["status"] == if i == 2 { "Failed" } else { "Succeeded" },
            "operation order and status",
        )?;
    }
    let no_write = json!({
        "created": false, "written_bytes": 0, "may_have_changed": false,
        "persistence_confirmed": false,
    });
    need(
        records[1]["stage"] == "Complete"
            && records[1]["target"] == PATH
            && records[1]["error"].is_null()
            && records[1]["partial_effect"]
                == json!({
                    "created": true, "written_bytes": BODY_LEN, "may_have_changed": true,
                    "persistence_confirmed": true,
                })
            && records[2]["stage"] == "Create"
            && records[2]["target"] == PATH
            && records[2]["partial_effect"] == no_write
            && records[2]["error"]
                == json!({
                    "code": "AlreadyExists", "stage": "Create", "object": PATH,
                    "cause": "Fat(AlreadyExists)", "partial_effect": no_write,
                })
            && records[2]["error"] == replies[4]["error"]["detail"],
        "creation and original conflict effects",
    )?;
    for event in &mut events {
        if let Event::Reply(reply) = event {
            normalize(reply, &boot, volume)?;
        }
    }
    Ok(events)
}

fn normalize(value: &mut Value, boot: &str, volume: &str) -> Result<()> {
    match value {
        Value::Array(items) => {
            for item in items {
                normalize(item, boot, volume)?;
            }
        }
        Value::Object(fields) => {
            for (key, value) in fields {
                match key.as_str() {
                    "boot_id" => {
                        need(*value == boot, "foreign boot identity")?;
                        *value = json!("<boot>");
                    }
                    "volume" => {
                        need(*value == volume, "foreign volume identity")?;
                        *value = json!("<volume>");
                    }
                    "operation_id" if !value.is_null() => {
                        let sequence = value
                            .as_str()
                            .and_then(|id| id.split_once(':'))
                            .filter(|(prefix, seq)| {
                                *prefix == boot && seq.parse::<u64>().is_ok_and(|n| n > 0)
                            })
                            .map(|(_, seq)| seq)
                            .ok_or_else(|| {
                                Error::new("at06-trace", "invalid operation identity")
                            })?;
                        *value = json!(format!("<boot>:{sequence}"));
                    }
                    _ => normalize(value, boot, volume)?,
                }
            }
        }
        _ => (),
    }
    Ok(())
}

fn host_trace(context: &Context, runner: &mut Runner) -> Result<Vec<Event>> {
    let fixture = runner.dir.join("at06-input.bin");
    fs::write(&fixture, input())?;
    let target = runner.dir.join("checks");
    let log = runner.run(
        context
            .cargo(&[
                "test",
                "--locked",
                "--offline",
                "-p",
                "radishroot-kernel",
                "--lib",
                "--target",
                &context.host,
            ])
            .arg("--target-dir")
            .arg(target)
            .args([
                "--",
                "shell::tests::at06_console_trace",
                "--exact",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("RADISHROOT_AT06_INPUT", fixture),
    )?;
    fs::write(runner.dir.join("at06-host.log"), &log)?;
    let events = trace(&log, HOST_BOOT)?;
    runner.record("at06_host=pass replies=8 capacity_events=2")?;
    Ok(events)
}

pub(crate) fn host(context: &Context, runner: &mut Runner) -> Result<()> {
    host_trace(context, runner)?;
    println!("AT06 host: command replies, input boundary and file effects passed");
    Ok(())
}

pub(crate) fn execute(
    context: &Context,
    runner: &mut Runner,
    manifest: &Manifest,
    machine: &Machine,
    source: &Path,
) -> Result<()> {
    let host = host_trace(context, runner)?;
    let disk = vm_storage::clone_sparse(source, runner)?;
    let guest = vm::boot(
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
    fs::write(runner.dir.join("at06-guest.log"), &guest)?;
    let ready = vm::fields(one(&guest, "RR:CONSOLE:READY ")?)?;
    let boot_id = ready
        .get("boot_id")
        .and_then(|id| u128::from_str_radix(id, 16).ok())
        .filter(|&id| id != 0)
        .ok_or_else(|| Error::new("at06-trace", "missing guest boot identity"))?
        .to_be_bytes();
    let guest = trace(&guest, boot_id)?;
    if let Some(index) = host.iter().zip(&guest).position(|(h, g)| h != g) {
        return Err(Error::new(
            "at06-equivalence",
            format!(
                "event {} differs; see at06-host.log and at06-guest.log",
                index + 1
            ),
        ));
    }
    vm_fat::tools_with_files(
        context,
        runner,
        manifest,
        &disk,
        FatCheck {
            mode: "verify",
            files: vec![Expected {
                id: "note",
                path: NOTE.into(),
                bytes: vec![b'x'; BODY_LEN],
            }],
            directories: Vec::new(),
        },
    )?;
    vm_fat::outside_unchanged(source, &disk, manifest)?;
    runner.record("at06_equivalence=pass replies=8 capacity_events=2 independent_DATA=pass outside_DATA=unchanged")?;
    println!("AT06: host and UART guest agree; independent DATA verification passed");
    Ok(())
}
