//! T05 编排与独立预期。mtools / dosfstools 不使用 guest FAT 库。
use crate::{
    boot_image::{self, Manifest},
    config::Context,
    error::{Error, Result},
    process::{self, Runner},
    vm::{self, BootSpec, fields, number},
    vm_storage,
};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::Command,
};

pub(super) struct Expected {
    pub id: &'static str,
    pub path: String,
    pub bytes: Vec<u8>,
}
fn expected_files() -> Vec<Expected> {
    let mut files = vec![
        ("seed", "seed.txt", "RadishRoot T05 seed v1\n"),
        ("case", "ReadMe.txt", "case-preserved\n"),
        ("unicode-case", "Straße.txt", "unicode case\n"),
        ("alias", "Long File Name.txt", "short alias\n"),
        ("composed", "é.txt", "composed\n"),
        ("decomposed", "e\u{301}.txt", "decomposed\n"),
        (
            "moved",
            "folder/moved.txt",
            "same-volume rename, exact bytes\n",
        ),
    ]
    .into_iter()
    .map(|(id, path, body)| Expected {
        id,
        path: path.into(),
        bytes: body.as_bytes().into(),
    })
    .collect::<Vec<_>>();
    files.push(Expected {
        id: "cross",
        path: "cross-sector.bin".into(),
        bytes: (0..4099).map(|i| ((i * 29 + 7) % 251) as u8).collect(),
    });
    files.push(Expected {
        id: "utf8",
        path: "萝卜 文件.txt".into(),
        bytes: "萝卜 UTF-8 persistence\n".repeat(97).into_bytes(),
    });
    files.push(Expected {
        id: "one-mib",
        path: "one-mib.bin".into(),
        bytes: (0..1048576).map(|i| ((i * 17 + 83) % 256) as u8).collect(),
    });
    files.push(Expected {
        id: "ascii-limit",
        path: "a".repeat(255),
        bytes: b"255 bytes\n".into(),
    });
    files.push(Expected {
        id: "utf8-limit",
        path: "卜".repeat(85),
        bytes: b"85 BMP characters\n".into(),
    });
    files
}
fn tools(
    context: &Context,
    runner: &mut Runner,
    manifest: &Manifest,
    disk: &Path,
    mode: &str,
    complete: bool,
) -> Result<PathBuf> {
    let files = expected_files()
        .into_iter()
        .filter(|e| complete || e.id == "seed")
        .collect();
    tools_with_files(
        context,
        runner,
        manifest,
        disk,
        FatCheck {
            mode,
            files,
            directories: if complete { vec!["folder"] } else { Vec::new() },
        },
    )
}
pub(super) struct FatCheck<'a> {
    pub mode: &'a str,
    pub files: Vec<Expected>,
    pub directories: Vec<&'a str>,
}
pub(super) fn tools_with_files(
    context: &Context,
    runner: &mut Runner,
    manifest: &Manifest,
    disk: &Path,
    check: FatCheck<'_>,
) -> Result<PathBuf> {
    let mode = check.mode;
    let expected = check.files;
    let directory = process::unique_directory(&runner.dir, "fat-tools")?;
    let inputs = directory.join("inputs");
    let output = directory.join("output");
    process::directory(&inputs)?;
    process::directory(&output)?;
    let mut index = String::new();
    for e in &expected {
        fs::write(inputs.join(format!("{}.bin", e.id)), &e.bytes)?;
        index.push_str(&format!("{}\t{}\n", e.id, e.path));
    }
    fs::write(inputs.join("files.tsv"), index)?;
    fs::copy(
        context.root.join("tools/boot/check-fat.sh"),
        inputs.join("check-fat.sh"),
    )?;
    let name = format!(
        "radishroot-{}",
        directory.file_name().unwrap().to_string_lossy()
    );
    let mut cmd = Command::new("docker");
    cmd.args([
        "run",
        "--rm",
        "--pull=never",
        "--name",
        &name,
        "--network=none",
        "--read-only",
        "--cpus=1",
        "--memory=256m",
        "--pids-limit=64",
        "--cap-drop=ALL",
        "--security-opt=no-new-privileges",
        "--tmpfs",
        "/tmp:rw,noexec,nosuid,size=16m",
        "--mount",
    ])
    .arg(format!(
        "type=bind,src={},dst=/inputs,readonly",
        inputs.display()
    ))
    .arg("--mount")
    .arg(format!(
        "type=bind,src={},dst=/disk/{}{}",
        disk.display(),
        "disk.raw",
        if mode == "prepare" { "" } else { ",readonly" }
    ))
    .arg("--mount")
    .arg(format!("type=bind,src={},dst=/output", output.display()))
    .args([
        manifest.get("debian_image")?,
        "/bin/sh",
        "/inputs/check-fat.sh",
        mode,
    ])
    .arg((manifest.number("data_start")? * 512).to_string())
    .arg(((manifest.number("last_lba")? + 1 - manifest.number("data_start")?) * 512).to_string());
    runner.record(&format!(
        "fat_tools={mode} disk={} guest_exited=true output={}",
        disk.display(),
        output.display()
    ))?;
    if let Err(error) = runner.run(&mut cmd) {
        let cleanup = runner.run(Command::new("docker").args(["rm", "-f", &name]));
        runner.record(&format!("container_cleanup={cleanup:?}"))?;
        return Err(error);
    }
    process::reject_output_symlinks(&directory)?;
    if mode == "verify" {
        let paths = fs::read_to_string(output.join("paths.txt"))?;
        let actual = paths.lines().map(String::from).collect::<BTreeSet<_>>();
        let mut wanted = expected
            .iter()
            .map(|e| format!("::/{}", e.path))
            .collect::<BTreeSet<_>>();
        for directory in check.directories {
            wanted.insert(format!("::/{directory}/"));
        }
        if actual != wanted {
            return Err(Error::new(
                "fat-paths",
                format!("expected={wanted:?} actual={actual:?}"),
            ));
        }
        for e in &expected {
            let path = output.join(format!("{}.bin", e.id));
            if fs::read(&path)? != e.bytes {
                return Err(Error::new("fat-content", format!("{} differs", e.path)));
            }
            crate::digest(&path, runner)?;
        }
        runner.record(&format!(
            "fat_independent=pass paths={} bytes=exact fsck_exit=0 tools=mtools,dosfstools",
            expected.len()
        ))?;
    }
    Ok(output)
}
pub(super) fn outside_unchanged(source: &Path, disk: &Path, manifest: &Manifest) -> Result<()> {
    let mut a = File::open(source)?;
    let mut b = File::open(disk)?;
    let length = manifest.number("disk_bytes")?;
    if a.metadata()?.len() != length || b.metadata()?.len() != length {
        return Err(Error::new("fat-boundary", "disk size changed"));
    }
    let mut expected = vec![0u8; 1024 * 1024];
    let mut actual = vec![0u8; expected.len()];
    for (start, end) in [
        (0, manifest.number("data_start")? * 512),
        ((manifest.number("last_lba")? + 1) * 512, length),
    ] {
        a.seek(SeekFrom::Start(start))?;
        b.seek(SeekFrom::Start(start))?;
        let mut at = start;
        while at < end {
            let n = (end - at).min(expected.len() as u64) as usize;
            a.read_exact(&mut expected[..n])?;
            b.read_exact(&mut actual[..n])?;
            if expected[..n] != actual[..n] {
                return Err(Error::new(
                    "fat-boundary",
                    format!("outside DATA changed at {at}"),
                ));
            }
            at += n as u64;
        }
    }
    Ok(())
}
pub(super) fn sha(path: &Path, runner: &mut Runner) -> Result<String> {
    let output = runner.run(Command::new("shasum").args(["-a", "256"]).arg(path))?;
    runner.record(output.trim())?;
    Ok(output.split_whitespace().next().unwrap_or("").into())
}
pub(super) fn execute(
    context: &Context,
    runner: &mut Runner,
    manifest: &Manifest,
    machine: &vm::Machine,
    kernel: &Path,
    source: &Path,
) -> Result<()> {
    let data = vm_storage::clone_sparse(source, runner)?;
    tools(context, runner, manifest, &data, "prepare", false)?;
    tools(context, runner, manifest, &data, "verify", false)?;
    outside_unchanged(source, &data, manifest)?;
    let mut before = sha(&data, runner)?;
    for mode in ["t05-write", "t05-reopen"] {
        let boot_disk = boot_image::create(&context.root, runner, manifest, kernel, mode)?;
        vm::boot(
            runner,
            manifest,
            machine,
            BootSpec {
                boot_disk: &boot_disk,
                data_disk: Some(&data),
                readonly: mode == "t05-reopen",
                mode,
            },
        )?;
        let after = sha(&data, runner)?;
        if mode == "t05-reopen" && before != after {
            return Err(Error::new("fat-readonly", "reopen modified disk"));
        }
        if mode == "t05-write" && before == after {
            return Err(Error::new("fat-write", "disk unchanged"));
        }
        before = after;
        tools(context, runner, manifest, &data, "verify", true)?;
        outside_unchanged(source, &data, manifest)?;
        crate::digest(&boot_disk, runner)?;
        println!("T05 {mode}: guest and independent FAT paths/length/bytes verified");
    }
    for mode in [
        "t05-readonly",
        "t05-file-sync",
        "t05-volume-sync",
        "t05-device-flush",
        "t05-bad-fat",
    ] {
        let data = vm_storage::clone_sparse(source, runner)?;
        tools(context, runner, manifest, &data, "prepare", false)?;
        if mode == "t05-bad-fat" {
            let mut f = OpenOptions::new().write(true).open(&data)?;
            for sector in [0, 6] {
                f.seek(SeekFrom::Start(
                    (manifest.number("data_start")? + sector) * 512 + 11,
                ))?;
                f.write_all(&[0, 0])?;
            }
            f.sync_all()?;
            fs::write(
                data.parent().unwrap().join("fat-fault.txt"),
                "DATA primary and backup BPB bytes_per_sector set to zero at offsets 11..13; no repair\n",
            )?;
        }
        let readonly = matches!(mode, "t05-readonly" | "t05-bad-fat");
        let before = sha(&data, runner)?;
        let boot_disk = boot_image::create(&context.root, runner, manifest, kernel, mode)?;
        vm::boot(
            runner,
            manifest,
            machine,
            BootSpec {
                boot_disk: &boot_disk,
                data_disk: Some(&data),
                readonly,
                mode,
            },
        )?;
        let after = sha(&data, runner)?;
        if readonly && before != after {
            return Err(Error::new(
                "fat-readonly",
                "rejected operation modified disk",
            ));
        }
        if mode == "t05-readonly" {
            tools(context, runner, manifest, &data, "verify", false)?;
        }
        if matches!(
            mode,
            "t05-file-sync" | "t05-volume-sync" | "t05-device-flush"
        ) {
            // 失败后只记录独立观察，不能把可读内容变成操作成功或持久化保证。
            tools(
                context,
                runner,
                manifest,
                &data,
                if mode == "t05-file-sync" {
                    "observe-file"
                } else {
                    "observe"
                },
                mode != "t05-file-sync",
            )?;
        }
        outside_unchanged(source, &data, manifest)?;
        crate::digest(&boot_disk, runner)?;
        runner.record(&format!("fat_failure={mode} disk={} readonly={readonly} outside_DATA_unchanged=true persistence=unconfirmed",data.display()))?;
        println!(
            "T05 {mode}: explicit failure, no success terminal, independent disk evidence retained"
        );
    }
    Ok(())
}

pub fn validate(output: &str, mode: &str) -> Result<()> {
    let lines = output
        .lines()
        .filter(|l| l.starts_with("RR:T05:"))
        .collect::<Vec<_>>();
    let count = |p: &str| lines.iter().filter(|l| l.starts_with(p)).count();
    let one = |p: &str| -> Result<_> {
        if count(p) != 1 {
            return Err(Error::new("fat-sequence", format!("expected one {p}")));
        }
        fields(lines.iter().find(|l| l.starts_with(p)).unwrap())
    };
    if lines.first() != Some(&"RR:T05:ENTER")
        || count("RR:T05:ENTER") != 1
        || count("RR:T05:CASE") != 1
        || lines.get(1) != Some(&format!("RR:T05:CASE name={mode}").as_str())
    {
        return Err(Error::new(
            "fat-sequence",
            "missing/duplicate entry or case",
        ));
    }
    let failure = match mode {
        "t05-readonly" => Some(("BLOCK_READ_ONLY", "Create")),
        "t05-file-sync" => Some(("FAT_WRITEBACK_INJECTED", "FileSync")),
        "t05-volume-sync" => Some(("FAT_WRITEBACK_INJECTED", "VolumeSync")),
        "t05-device-flush" => Some(("BLOCK_UNSUPPORTED", "DeviceFlush")),
        "t05-bad-fat" => Some(("FAT_CORRUPT", "Mount")),
        "t05-write" | "t05-reopen" => None,
        _ => return Err(Error::new("fat-case", "unknown case")),
    };
    let readonly = matches!(mode, "t05-reopen" | "t05-readonly" | "t05-bad-fat");
    let volume = one("RR:T05:VOLUME ")?;
    if number(&volume, "readonly")? != u64::from(readonly)
        || number(&volume, "degraded")? != 0
        || number(&volume, "data_start")? != 264192
        || number(&volume, "data_sectors")? != 1832927
    {
        return Err(Error::new("fat-volume", "wrong volume or permissions"));
    }
    let io = one("RR:T05:IO ")?;
    let stop = one("RR:T05:STOP ")?;
    if stop.get("reset") != Some(&"confirmed")
        || number(&stop, "bus_master")? != 0
        || number(&stop, "allocations")? != 0
        || number(&stop, "shared")? != 0
        || number(&stop, "requests")? == 0
    {
        return Err(Error::new("fat-stop", "missing reset / resource release"));
    }
    if let Some((code, phase)) = failure {
        if count("RR:T05:READY") != 0
            || count("RR:T05:FAIL") != 1
            || io.get("persistence") != Some(&"unconfirmed")
            || lines.last() != Some(&format!("RR:T05:FAIL code={code} stage=fat").as_str())
        {
            return Err(Error::new(
                "fat-failure",
                "wrong terminal or persistence claim",
            ));
        }
        if mode != "t05-bad-fat"
            && (number(&io, "first_error")? != 1
                || !lines.iter().any(|l| {
                    l.starts_with("RR:T05:FIRST_ERROR ") && l.contains(&format!("phase: {phase},"))
                }))
        {
            return Err(Error::new("fat-failure", "first cause/phase lost"));
        }
        if matches!(mode, "t05-file-sync" | "t05-volume-sync" | "t05-readonly")
            && number(&io, "blocked")? == 0
        {
            return Err(Error::new("fat-failure", "destructor retries not blocked"));
        }
        if mode == "t05-file-sync" && number(&io, "written")? != 1649 {
            return Err(Error::new("fat-progress", "wrong confirmed file bytes"));
        }
        if readonly && (number(&io, "written")? != 0 || number(&stop, "flushes")? != 0) {
            return Err(Error::new("fat-readonly", "mutating request"));
        }
        if mode == "t05-device-flush"
            && !output.contains("mechanism=unsupported-request response=real")
        {
            return Err(Error::new(
                "fat-injection",
                "missing actual negative response mechanism",
            ));
        }
        return Ok(());
    }
    if count("RR:T05:FAIL") != 0
        || count("RR:T05:READY") != 1
        || lines.last() != Some(&format!("RR:T05:READY mode={mode}").as_str())
        || number(&io, "first_error")? != 0
        || number(&io, "blocked")? != 0
        || io.get("persistence") != Some(&"confirmed")
    {
        return Err(Error::new(
            "fat-sequence",
            "unexpected terminal or hidden IO error",
        ));
    }
    let expected = expected_files();
    if count("RR:T05:FILE ") != expected.len() {
        return Err(Error::new("fat-files", "wrong file count"));
    }
    for e in &expected {
        let f = one(&format!("RR:T05:FILE id={} ", e.id))?;
        if number(&f, "length")? != e.bytes.len() as u64 || number(&f, "verified")? != 1 {
            return Err(Error::new("fat-files", format!("wrong result {}", e.path)));
        }
    }
    let sync = lines
        .iter()
        .filter(|l| l.starts_with("RR:T05:SYNC "))
        .copied()
        .collect::<Vec<_>>();
    let last = format!(
        "RR:T05:SYNC stage=DeviceFlush result=ok readonly={}",
        u8::from(readonly)
    );
    if sync
        != [
            "RR:T05:SYNC stage=VolumeSync result=ok",
            "RR:T05:SYNC stage=Unmount result=ok",
            last.as_str(),
        ]
    {
        return Err(Error::new("fat-sync", "wrong synchronization order"));
    }
    let written = if readonly {
        0
    } else {
        expected
            .iter()
            .filter(|e| e.id != "seed")
            .map(|e| e.bytes.len() as u64)
            .sum()
    };
    if number(&io, "written")? != written
        || (readonly && number(&stop, "flushes")? != 0)
        || (!readonly && number(&stop, "flushes")? == 0)
    {
        return Err(Error::new(
            "fat-progress",
            "wrong byte count or device flush",
        ));
    }
    if !readonly && !lines.contains(&"RR:T05:NAMES collisions=5 invalid=8 same_dir_rename=1 cross_dir_file_rename=1 case_only_rename=no-op normalization=none") {return Err(Error::new("fat-names","missing name semantics"));}
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failure_cannot_publish_success_or_lose_first_phase() {
        let text = "RR:T05:ENTER\nRR:T05:CASE name=t05-file-sync\nRR:T05:VOLUME readonly=0 degraded=0 data_start=264192 data_sectors=1832927\nRR:T05:IO calls=100 blocked=3 written=1649 first_error=1 persistence=unconfirmed\nRR:T05:FIRST_ERROR detail=Error { phase: FileSync, cause: InjectedWrite, offset: 1 }\nRR:T05:STOP reset=confirmed bus_master=0 allocations=0 shared=0 requests=100 flushes=3\nRR:T05:FAIL code=FAT_WRITEBACK_INJECTED stage=fat\n";
        assert!(validate(text, "t05-file-sync").is_ok());
        for modified in [
            text.replace("phase: FileSync", "phase: Unmount"),
            text.replace("blocked=3", "blocked=0"),
            text.replace("written=1649", "written=0"),
            format!("{text}RR:T05:READY mode=t05-file-sync\n"),
        ] {
            assert!(validate(&modified, "t05-file-sync").is_err());
        }
    }
}
