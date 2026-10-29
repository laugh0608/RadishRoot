//! T08 服务验证与独立 mtools 读取。期望由宿主合成，不调用 guest FAT 实现。
use crate::{
    boot_image::{self, Manifest},
    config::Context,
    error::{Error, Result},
    process::Runner,
    vm::{self, BootSpec, Machine, fields, number},
    vm_fat::{self, Expected, FatCheck},
    vm_storage,
};
use std::{
    fs::{self, OpenOptions},
    io::{Seek, SeekFrom, Write},
    path::Path,
};
const CASES: [&str; 7] = [
    "t08-write",
    "t08-reopen",
    "t08-readonly",
    "t08-file-sync",
    "t08-volume-sync",
    "t08-device-flush",
    "t08-bad-fat",
];
fn expected(complete: bool, failed: bool) -> Vec<Expected> {
    let mut v = vec![Expected {
        id: "seed",
        path: "seed.txt".into(),
        bytes: b"RadishRoot T08 seed\n".to_vec(),
    }];
    if complete {
        v.push(Expected {
            id: "service",
            path: if failed {
                "failed.txt"
            } else {
                "萝卜 服务.txt"
            }
            .into(),
            bytes: b"RadishRoot T08 shared service\n".repeat(57),
        });
    }
    v
}
fn tools(
    c: &Context,
    r: &mut Runner,
    m: &Manifest,
    d: &Path,
    mode: &str,
    complete: bool,
    failed: bool,
) -> Result<()> {
    vm_fat::tools_with_files(
        c,
        r,
        m,
        d,
        FatCheck {
            mode,
            files: expected(complete, failed),
            directories: Vec::new(),
        },
    )?;
    Ok(())
}
pub(super) fn execute(
    c: &Context,
    r: &mut Runner,
    m: &Manifest,
    machine: &Machine,
    kernel: &Path,
    source: &Path,
) -> Result<()> {
    let shared = vm_storage::clone_sparse(source, r)?;
    tools(c, r, m, &shared, "prepare", false, false)?;
    for mode in CASES {
        let data = if matches!(mode, "t08-write" | "t08-reopen") {
            shared.clone()
        } else {
            let d = vm_storage::clone_sparse(source, r)?;
            tools(c, r, m, &d, "prepare", false, false)?;
            d
        };
        if mode == "t08-bad-fat" {
            let mut f = OpenOptions::new().write(true).open(&data)?;
            for sector in [0, 6] {
                f.seek(SeekFrom::Start(
                    (m.number("data_start")? + sector) * 512 + 11,
                ))?;
                f.write_all(&[0, 0])?;
            }
            f.sync_all()?;
            fs::write(
                data.parent().unwrap().join("fat-fault.txt"),
                "T08: DATA primary/backup BPB bytes_per_sector=0; no repair\n",
            )?;
        }
        let readonly = matches!(mode, "t08-reopen" | "t08-readonly" | "t08-bad-fat");
        let before = vm_fat::sha(&data, r)?;
        let boot = boot_image::create(&c.root, r, m, kernel, mode)?;
        let output = vm::boot(
            r,
            m,
            machine,
            BootSpec {
                boot_disk: &boot,
                data_disk: Some(&data),
                readonly,
                mode,
            },
        )?;
        // 删除真实证据字段的负对照必须被同一个验收器拒绝。
        let id_path = r
            .dir
            .read_dir()?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.join("serial.log").exists())
            .find(|p| fs::read_to_string(p.join("serial.log")).is_ok_and(|s| s == output))
            .ok_or_else(|| Error::new("service-evidence", "missing serial directory"))?;
        let id: [u8; 16] = fs::read(id_path.join("boot-id.bin"))?
            .try_into()
            .map_err(|_| Error::new("service-identity", "length"))?;
        for (label, bad) in [
            (
                "missing-stop",
                output
                    .lines()
                    .filter(|s| !s.starts_with("RR:T08:STOP"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            (
                "duplicate-ready",
                format!("{output}RR:T08:READY mode={mode}\n"),
            ),
            (
                "wrong-id",
                output.replace(
                    &format!("boot_id={:032x}", u128::from_be_bytes(id)),
                    "boot_id=0",
                ),
            ),
        ] {
            if validate(&bad, mode, id).is_ok() {
                return Err(Error::new("service-negative", label));
            }
            fs::write(id_path.join(format!("negative-{label}.log")), bad)?;
        }
        let after = vm_fat::sha(&data, r)?;
        if readonly && before != after {
            return Err(Error::new("service-readonly", "disk changed"));
        }
        if mode == "t08-write" && before == after {
            return Err(Error::new("service-write", "disk unchanged"));
        }
        match mode {
            "t08-write" | "t08-reopen" => tools(c, r, m, &data, "verify", true, false)?,
            "t08-readonly" => tools(c, r, m, &data, "verify", false, false)?,
            "t08-file-sync" => tools(c, r, m, &data, "observe-file", false, true)?,
            "t08-volume-sync" | "t08-device-flush" => tools(c, r, m, &data, "observe", true, true)?,
            _ => (),
        }
        vm_fat::outside_unchanged(source, &data, m)?;
        r.record(&format!("service_case={mode} independent_DATA=checked outside_DATA=unchanged readonly={readonly} disk={}",data.display()))?;
        println!("T08 {mode}: guest service and independent disk verification passed");
    }
    Ok(())
}
fn bad(s: &str) -> Error {
    Error::new("service-contract", s)
}
fn one<'a>(output: &'a str, prefix: &str) -> Result<&'a str> {
    let lines: Vec<_> = output.lines().filter(|l| l.starts_with(prefix)).collect();
    if lines.len() != 1 {
        return Err(bad(prefix));
    }
    Ok(lines[0])
}
pub fn validate_default(output: &str, id: [u8; 16]) -> Result<()> {
    let f = fields(one(output, "RR:T08:DEFAULT ")?)?;
    if f.get("boot_id") != Some(&format!("{:032x}", u128::from_be_bytes(id)).as_str())
        || number(&f, "entries")? > 32
        || number(&f, "more")? > 1
        || number(&f, "readonly")? != 1
        || number(&f, "generation")? != 0
        || number(&f, "operation")? != 1
        || output.contains("RR:T08:FAIL")
    {
        return Err(bad("default service facts"));
    }
    if one(output, "RR:T08:AVAILABLE ")? != "RR:T08:AVAILABLE generation=0 state=Ready" {
        return Err(bad("default service unavailable"));
    }
    Ok(())
}
pub fn validate(output: &str, mode: &str, id: [u8; 16]) -> Result<()> {
    if !CASES.contains(&mode) || id == [0; 16] {
        return Err(bad("mode/identity"));
    }
    if one(output, "RR:T08:ENTER ")?
        != format!(
            "RR:T08:ENTER mode={mode} boot_id={:032x}",
            u128::from_be_bytes(id)
        )
        || one(output, "RR:T08:READY ")? != format!("RR:T08:READY mode={mode}")
        || output
            .lines()
            .any(|l| l.starts_with("RR:T") && l.contains(":FAIL "))
    {
        return Err(bad("terminal"));
    }
    let lines: Vec<_> = output
        .lines()
        .filter(|l| l.starts_with("RR:T08:"))
        .collect();
    if !lines.first().unwrap().starts_with("RR:T08:ENTER ")
        || !lines.last().unwrap().starts_with("RR:T08:READY ")
    {
        return Err(bad("terminal order"));
    }
    let stop = fields(one(output, "RR:T08:STOP ")?)?;
    if stop.get("reset") != Some(&"confirmed")
        || number(&stop, "bus_master")? != 0
        || number(&stop, "allocations")? != 0
        || number(&stop, "shared")? != 0
        || number(&stop, "requests")? == 0
    {
        return Err(bad("stop"));
    }
    let partial = matches!(
        mode,
        "t08-file-sync" | "t08-volume-sync" | "t08-device-flush"
    );
    let success = matches!(mode, "t08-write" | "t08-reopen");
    let result = fields(one(output, "RR:T08:RESULT ")?)?;
    let changed = partial || mode == "t08-write";
    if number(&result, "generation")? != u64::from(changed)
        || number(&result, "changes")? != u64::from(changed)
        || number(&result, "expected_failure")? != u64::from(!success)
        || result.get("state")
            != Some(&if partial || mode == "t08-bad-fat" {
                "Offline"
            } else {
                "Ready"
            })
    {
        return Err(bad("result facts"));
    }
    let ops = lines
        .iter()
        .filter(|l| l.starts_with("RR:T08:OP "))
        .map(|l| fields(l))
        .collect::<Result<Vec<_>>>()?;
    if ops.len() as u64 != number(&result, "records")? || ops.is_empty() {
        return Err(bad("record count"));
    }
    for (i, op) in ops.iter().enumerate() {
        if number(op, "id")? != i as u64 + 1
            || !matches!(op.get("status"), Some(&"Succeeded" | &"Failed"))
        {
            return Err(bad("record identity/status"));
        }
    }
    let expected_ops: Vec<(&str, &str, &str, &str)> = match mode {
        "t08-write" => vec![
            ("List", "Complete", "Succeeded", "Some(Complete)"),
            ("Read", "Complete", "Succeeded", "Some(Complete)"),
            ("CreateText", "Validate", "Failed", "None"),
            ("CreateText", "Validate", "Failed", "None"),
            ("CreateText", "Validate", "Failed", "None"),
            ("CreateText", "Complete", "Succeeded", "Some(Complete)"),
            ("CreateText", "Create", "Failed", "Some(Mount)"),
            ("Read", "Validate", "Failed", "None"),
            ("Read", "Complete", "Succeeded", "Some(Complete)"),
            ("Read", "Read", "Failed", "Some(Mount)"),
            ("Read", "Complete", "Succeeded", "Some(Complete)"),
        ],
        "t08-reopen" => vec![
            ("List", "Complete", "Succeeded", "Some(Complete)"),
            ("Read", "Complete", "Succeeded", "Some(Complete)"),
            ("Read", "Complete", "Succeeded", "Some(Complete)"),
            ("Read", "Read", "Failed", "Some(Mount)"),
            ("Read", "Complete", "Succeeded", "Some(Complete)"),
        ],
        "t08-readonly" => vec![
            ("CreateText", "Validate", "Failed", "None"),
            ("Read", "Complete", "Succeeded", "Some(Complete)"),
        ],
        "t08-bad-fat" => vec![
            ("List", "Mount", "Failed", "Some(Validate)"),
            ("List", "Validate", "Failed", "None"),
        ],
        _ => vec![
            (
                "CreateText",
                if mode == "t08-file-sync" {
                    "FileSync"
                } else if mode == "t08-volume-sync" {
                    "VolumeSync"
                } else {
                    "DeviceFlush"
                },
                "Failed",
                if mode == "t08-file-sync" {
                    "Some(Write)"
                } else if mode == "t08-volume-sync" {
                    "Some(FileSync)"
                } else {
                    "Some(Unmount)"
                },
            ),
            ("List", "Validate", "Failed", "None"),
        ],
    };
    if ops.len() != expected_ops.len() {
        return Err(bad("operation sequence length"));
    }
    for (op, (kind, stage, status, last)) in ops.iter().zip(expected_ops) {
        if op.get("kind") != Some(&kind)
            || op.get("stage") != Some(&stage)
            || op.get("status") != Some(&status)
            || op.get("last") != Some(&last)
        {
            return Err(bad("operation facts / last completed stage"));
        }
    }
    for (prefix, count) in [
        ("RR:T08:ERROR ", usize::from(!success)),
        ("RR:T08:CREATE ", usize::from(mode == "t08-write")),
        ("RR:T08:INJECTION ", usize::from(partial)),
        ("RR:T08:CONTENT ", usize::from(success)),
    ] {
        if lines.iter().filter(|line| line.starts_with(prefix)).count() != count {
            return Err(bad("unexpected or missing service evidence"));
        }
    }
    if !success {
        let line = one(output, "RR:T08:ERROR ")?;
        // cause 是有类型的调试结构，基础字段在 cause 前严格解析。
        let e = fields(line.split(" cause=").next().unwrap())?;
        let (code, stage) = match mode {
            "t08-readonly" => ("ReadOnly", "Validate"),
            "t08-bad-fat" => ("Corrupt", "Mount"),
            "t08-file-sync" => ("Io", "FileSync"),
            "t08-volume-sync" => ("Io", "VolumeSync"),
            _ => ("Io", "DeviceFlush"),
        };
        if e.get("code") != Some(&code)
            || e.get("stage") != Some(&stage)
            || number(&e, "operation")? != 1
            || number(&e, "created")? != u64::from(partial)
            || number(&e, "changed")? != u64::from(partial)
            || number(&e, "written")? != if partial { 1710 } else { 0 }
            || number(&e, "persistence")? != 0
            || ops[0].get("status") != Some(&"Failed")
            || ops[0].get("stage") != Some(&stage)
        {
            return Err(bad("failure facts"));
        }
        if partial
            && (!line.contains(&format!("phase: {stage},"))
                || !line.contains(if mode == "t08-device-flush" {
                    "InjectedFlush"
                } else {
                    "InjectedWrite"
                }))
        {
            return Err(bad("original cause"));
        }
        if partial
            && one(output, "RR:T08:INJECTION ")?
                != format!("RR:T08:INJECTION stage={stage} mechanism=adapter-once")
        {
            return Err(bad("injection"));
        }
        if output.contains("RR:T08:CREATE ") {
            return Err(bad("false create success"));
        }
    } else {
        if one(output, "RR:T08:CONTENT ")?
            != "RR:T08:CONTENT files=2 bytes=1710 missing_recovered=1"
        {
            return Err(bad("content"));
        }
        if mode == "t08-write" {
            if one(output, "RR:T08:CREATE ")?
                != "RR:T08:CREATE operation=6 written=1710 flush_order=FileSync,VolumeSync,Unmount,DeviceFlush persistence=1 conflicts=1 stale=1 invalid=3"
            {
                return Err(bad("create evidence"));
            }
        } else if output.contains("RR:T08:CREATE ") {
            return Err(bad("reopen mutation"));
        }
    }
    if matches!(mode, "t08-reopen" | "t08-readonly" | "t08-bad-fat")
        && number(&stop, "flushes")? != 0
    {
        return Err(bad("readonly flushed"));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failure_record_identity_persistence_and_stop_are_required() {
        let id = [1; 16];
        let text = format!(
            "RR:T08:ENTER mode=t08-readonly boot_id={:032x}\nRR:T08:ERROR operation=1 code=ReadOnly stage=Validate created=0 written=0 changed=0 persistence=0 cause=Contract(ReadOnly)\nRR:T08:RESULT generation=0 changes=0 records=2 state=Ready expected_failure=1\nRR:T08:OP id=1 kind=CreateText stage=Validate status=Failed last=None\nRR:T08:OP id=2 kind=Read stage=Complete status=Succeeded last=Some(Complete)\nRR:T08:STOP reset=confirmed bus_master=0 allocations=0 shared=0 requests=20 flushes=0\nRR:T08:READY mode=t08-readonly\n",
            u128::from_be_bytes(id)
        );
        assert!(validate(&text, "t08-readonly", id).is_ok());
        for bad in [
            text.replace("persistence=0", "persistence=1"),
            text.replace("status=Failed", "status=Succeeded"),
            text.replace("reset=confirmed", "reset=unknown"),
            text.replace("flushes=0", "flushes=1"),
            text.replace("records=2", "records=1"),
            format!("{text}RR:T08:READY mode=t08-readonly\n"),
        ] {
            assert!(validate(&bad, "t08-readonly", id).is_err());
        }
        assert!(validate(&text, "t08-readonly", [2; 16]).is_err());
    }
}

#[cfg(test)]
mod acceptance_regression {
    use super::*;
    #[test]
    fn payload_length_is_an_independent_byte_expectation() {
        let files = expected(true, false);
        assert_eq!(files[1].bytes.len(), 1710);
        let (chunks, remainder) = files[1].bytes.as_chunks::<30>();
        assert_eq!(chunks.len(), 57);
        assert!(remainder.is_empty());
        assert!(
            chunks
                .iter()
                .all(|c| c == b"RadishRoot T08 shared service\n")
        );
    }
}
