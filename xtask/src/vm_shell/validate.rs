use super::script;
use crate::{
    error::{Error, Result},
    vm::{fields, number},
};
use serde_json::Value;
fn bad(detail: &str) -> Error {
    Error::new("shell-contract", detail)
}
fn require(ok: bool, detail: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(bad(detail)) }
}
fn one<'a>(s: &'a str, p: &str) -> Result<&'a str> {
    let lines: Vec<_> = s.lines().filter(|l| l.starts_with(p)).collect();
    if lines.len() != 1 {
        return Err(bad(p));
    }
    Ok(lines[0])
}
pub fn validate_default(s: &str, id: [u8; 16]) -> Result<()> {
    require(
        one(s, "RR:CONSOLE:READY ")?
            == format!(
                "RR:CONSOLE:READY boot_id={:032x} line_bytes=1024 history=16 cwd=/data",
                u128::from_be_bytes(id)
            ),
        "console identity/capacity",
    )?;
    require(
        !s.contains("RR:T09:FAIL") && !s.contains("RR:CONSOLE:TRANSPORT"),
        "console failed",
    )
}
fn bytes(r: &Value, expected: &[u8]) -> Result<()> {
    let o = &r["output"];
    let hex: String = expected.iter().map(|b| format!("{b:02x}")).collect();
    require(
        o["kind"] == "bytes"
            && o["offset"] == 0
            && o["length"] == expected.len()
            && o["eof"] == true
            && o["hex"] == hex,
        "file byte result",
    )?;
    require(
        o["text"] == std::str::from_utf8(expected).unwrap(),
        "file text projection",
    )
}
fn error(r: &Value, code: &str, stage: &str) -> Result<()> {
    require(
        r["ok"] == false
            && r["error"]["kind"] == "service"
            && r["error"]["detail"]["code"] == code
            && r["error"]["detail"]["stage"] == stage,
        "service error identity/stage",
    )
}
fn explanation(r: &Value, record: &Value) -> Result<()> {
    let o = &r["output"];
    require(
        o["kind"] == "explanation" && o["record"] == *record && o["events_evicted"] == 0,
        "same operation detail",
    )?;
    let events = o["events"].as_array().ok_or_else(|| bad("events array"))?;
    require(
        !events.is_empty()
            && events.last().unwrap()["status"] == record["status"]
            && events.last().unwrap()["stage"] == record["stage"],
        "last event state",
    )?;
    require(
        events.windows(2).all(|p| {
            p[0]["seq"]
                .as_u64()
                .zip(p[1]["seq"].as_u64())
                .is_some_and(|(a, b)| a + 1 == b)
        }),
        "event order",
    )
}
pub fn validate(s: &str, mode: &str, id: [u8; 16]) -> Result<()> {
    let input = script(mode, id)?;
    validate_default(s, id)?;
    require(
        one(s, "RR:T09:ENTER ")?
            == format!(
                "RR:T09:ENTER mode={mode} boot_id={:032x}",
                u128::from_be_bytes(id)
            ),
        "enter identity",
    )?;
    require(
        one(s, "RR:T09:READY ")? == format!("RR:T09:READY mode={mode}")
            && s.lines().last() == Some(format!("RR:T09:READY mode={mode}").as_str()),
        "ready terminal",
    )?;
    require(
        !s.lines()
            .any(|l| l.starts_with("RR:T") && l.contains(":FAIL ")),
        "guest failure",
    )?;
    let acknowledgements = s
        .lines()
        .filter_map(|l| l.strip_prefix("RR:CONSOLE:INPUT seq="))
        .map(|s| s.parse::<usize>().map_err(|_| bad("input sequence")))
        .collect::<Result<Vec<_>>>()?;
    require(
        acknowledgements == (1..=input.len()).collect::<Vec<_>>(),
        "all physical input bytes acknowledged",
    )?;
    let replies = s
        .lines()
        .filter_map(|l| l.strip_prefix("RR:CONSOLE:REPLY "))
        .map(|s| serde_json::from_str::<Value>(s).map_err(|_| bad("reply JSON")))
        .collect::<Result<Vec<_>>>()?;
    let expected_ops: &[u64] = match mode {
        "t09-write" => &[0, 1, 0, 0, 2, 3, 4, 5, 0, 0, 0, 6, 7, 8, 9, 0, 0, 10, 11, 0],
        "t09-reopen" => &[0, 1, 2, 3, 4, 0],
        _ => &[1, 0, 2, 0],
    };
    require(replies.len() == expected_ops.len(), "reply count")?;
    for (i, (r, op)) in replies.iter().zip(expected_ops).enumerate() {
        require(
            r["schema"] == "radishroot.command.v1" && r["command_id"] == i + 1,
            "reply schema/id",
        )?;
        require(
            if *op == 0 {
                r["operation_id"].is_null()
            } else {
                r["operation_id"] == format!("{:032x}:{op}", u128::from_be_bytes(id))
            },
            "operation id",
        )?;
        require(r["ok"].is_boolean(), "typed success")?;
    }
    let successful: &[usize] = match mode {
        "t09-write" => &[0, 1, 5, 7, 8, 9, 10, 11, 12, 13, 14, 16, 18],
        "t09-reopen" => &[0, 1, 2, 3, 4, 5],
        "t09-readonly" => &[1, 2, 3],
        _ => &[1, 3],
    };
    for (i, r) in replies.iter().enumerate() {
        require(
            r["ok"] == successful.contains(&i),
            "expected command success/failure",
        )?;
    }
    let stop = fields(one(s, "RR:T09:STOP ")?)?;
    require(
        stop.get("reset") == Some(&"confirmed")
            && ["bus_master", "allocations", "shared"]
                .iter()
                .all(|k| number(&stop, k).is_ok_and(|n| n == 0))
            && number(&stop, "requests")? > 0,
        "device stop",
    )?;
    if matches!(mode, "t09-readonly" | "t09-reopen" | "t09-bad-fat") {
        require(number(&stop, "flushes")? == 0, "readonly flush")?;
    }
    let (records, generation, state) = match mode {
        "t09-write" => (11, 2, "Ready"),
        "t09-reopen" => (4, 0, "Ready"),
        "t09-readonly" => (2, 0, "Ready"),
        "t09-bad-fat" => (2, 0, "Offline"),
        _ => (2, 1, "Offline"),
    };
    let result = fields(one(s, "RR:T09:RESULT ")?)?;
    require(
        number(&result, "commands")? == replies.len() as u64
            && number(&result, "records")? == records
            && number(&result, "generation")? == generation
            && number(&result, "changes")? == generation
            && result.get("state") == Some(&state),
        "service summary",
    )?;
    if mode == "t09-write" {
        require(
            replies[0]["output"]["kind"] == "help"
                && replies[0]["output"]["commands"]
                    .as_array()
                    .is_some_and(|a| a.len() == 6)
                && replies[16]["output"] == replies[0]["output"],
            "editing/history help",
        )?;
        require(
            replies[2]["error"]["kind"] == "parse"
                && replies[2]["error"]["code"] == "Arguments"
                && replies[3]["error"]["code"] == "Quote",
            "parse rejects",
        )?;
        error(&replies[4], "InvalidPath", "Validate")?;
        error(&replies[6], "AlreadyExists", "Create")?;
        error(&replies[17], "NotFound", "Read")?;
        require(
            replies[15]["error"]["kind"] == "ForeignBoot"
                && replies[19]["error"]["kind"] == "NoNextPage",
            "identity/pagination refusal",
        )?;
        for i in [5, 11] {
            require(
                replies[i]["output"]["kind"] == "created"
                    && replies[i]["output"]["reference"]["generation"]
                        == if i == 5 { 1 } else { 2 },
                "creation reference",
            )?;
        }
        bytes(&replies[7], b"hello \"quoted\"\n")?;
        bytes(&replies[12], "中文 value".as_bytes())?;
        bytes(&replies[14], b"RadishRoot T09 seed\n")?;
        bytes(&replies[18], b"RadishRoot T09 seed\n")?;
        let ops = replies[8]["output"]["records"]
            .as_array()
            .ok_or_else(|| bad("operations"))?;
        require(
            ops.len() == 5 && replies[8]["output"]["evicted"] == 0,
            "operation snapshot",
        )?;
        explanation(&replies[9], &ops[2])?;
        explanation(&replies[10], &ops[3])?;
        require(
            ops[2]["status"] == "Succeeded"
                && ops[2]["partial_effect"]["persistence_confirmed"] == true
                && ops[2]["partial_effect"]["written_bytes"] == 15
                && ops[3]["status"] == "Failed"
                && ops[3]["error"] == replies[6]["error"]["detail"],
            "success/conflict details",
        )?;
        let flushes = replies[9]["output"]["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["status"] == "Flushing")
            .map(|e| e["stage"].as_str().unwrap_or(""))
            .collect::<Vec<_>>();
        require(
            flushes == ["FileSync", "VolumeSync", "Unmount", "DeviceFlush"],
            "flush order",
        )?;
        require(
            s.lines()
                .filter(|l| l.starts_with("RR:CONSOLE:INPUT_ERROR code=Utf8 "))
                .count()
                == 2,
            "damaged line rejection",
        )?;
        for (i, count) in [(1, 1), (13, 3)] {
            require(
                replies[i]["output"]["kind"] == "files"
                    && replies[i]["output"]["entries"]
                        .as_array()
                        .is_some_and(|v| v.len() == count)
                    && replies[i]["output"]["more"] == false,
                "directory count/more",
            )?;
        }
    } else if mode == "t09-reopen" {
        bytes(&replies[2], b"hello \"quoted\"\n")?;
        bytes(&replies[3], "中文 value".as_bytes())?;
        bytes(&replies[4], b"RadishRoot T09 seed\n")?;
        require(
            replies[1]["output"]["entries"]
                .as_array()
                .is_some_and(|v| v.len() == 3)
                && replies[5]["output"]["records"]
                    .as_array()
                    .is_some_and(|v| v.len() == 4),
            "reopened counts",
        )?;
    } else {
        let (code, stage) = match mode {
            "t09-readonly" => ("ReadOnly", "Validate"),
            "t09-bad-fat" => ("Corrupt", "Mount"),
            "t09-file-sync" => ("Io", "FileSync"),
            "t09-volume-sync" => ("Io", "VolumeSync"),
            _ => ("Io", "DeviceFlush"),
        };
        error(&replies[0], code, stage)?;
        let ops = replies[3]["output"]["records"]
            .as_array()
            .ok_or_else(|| bad("failure operations"))?;
        require(ops.len() == 2, "failure record count")?;
        explanation(&replies[1], &ops[0])?;
        require(
            ops[0]["error"] == replies[0]["error"]["detail"],
            "original error preserved",
        )?;
        let partial = mode.contains("sync") || mode == "t09-device-flush";
        let effect = &ops[0]["partial_effect"];
        require(
            effect["created"] == partial
                && effect["may_have_changed"] == partial
                && effect["written_bytes"] == if partial { 16 } else { 0 }
                && effect["persistence_confirmed"] == false,
            "failure partial effects",
        )?;
        if mode == "t09-readonly" {
            bytes(&replies[2], b"RadishRoot T09 seed\n")?
        } else {
            error(&replies[2], "Offline", "Validate")?
        }
        if partial {
            require(
                one(s, "RR:T09:INJECTION ")?
                    == format!("RR:T09:INJECTION stage={stage} mechanism=adapter-once"),
                "injection mechanism",
            )?;
        }
    }
    Ok(())
}
