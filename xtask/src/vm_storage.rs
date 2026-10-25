use crate::{
    boot_image::{self, Manifest},
    config::Context,
    error::{Error, Result},
    process::{self, Runner},
    vm::{fields, number},
};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

struct Patch {
    offset: u64,
    bytes: Vec<u8>,
}
pub(super) fn clone_sparse(source: &Path, runner: &mut Runner) -> Result<PathBuf> {
    if !fs::symlink_metadata(source)?.is_file() || fs::metadata(source)?.len() != 1073741824 {
        return Err(Error::new(
            "storage-fixture",
            "source must be owned 1 GiB regular image",
        ));
    }
    let directory = process::unique_directory(&runner.dir, "data")?;
    let target = directory.join("disk.raw");
    let mut input = File::open(source)?;
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&target)?;
    output.set_len(1073741824)?;
    let mut block = vec![0; 1024 * 1024];
    for offset in (0..1073741824u64).step_by(block.len()) {
        input.read_exact(&mut block)?;
        if block.iter().any(|b| *b != 0) {
            output.seek(SeekFrom::Start(offset))?;
            output.write_all(&block)?;
        }
    }
    output.sync_all()?;
    fs::write(
        directory.join("source.txt"),
        format!("{}\n", source.display()),
    )?;
    runner.record(&format!(
        "storage_clone={} source={} sparse=true",
        target.display(),
        source.display()
    ))?;
    Ok(target)
}
fn apply_patches(path: &Path, patches: &[Patch], label: &str) -> Result<()> {
    let mut file = OpenOptions::new().read(true).write(true).open(path)?;
    let length = file.metadata()?.len();
    let mut report = String::new();
    for (i, patch) in patches.iter().enumerate() {
        if patch
            .offset
            .checked_add(patch.bytes.len() as u64)
            .is_none_or(|end| end > length)
        {
            return Err(Error::new("storage-fixture", "patch outside image"));
        }
        file.seek(SeekFrom::Start(patch.offset))?;
        file.write_all(&patch.bytes)?;
        let name = format!("{label}-{i}.bin");
        fs::write(path.parent().unwrap().join(&name), &patch.bytes)?;
        report.push_str(&format!(
            "{}\t{}\t{name}\n",
            patch.offset,
            patch.bytes.len()
        ));
    }
    file.sync_all()?;
    fs::write(path.parent().unwrap().join(format!("{label}.tsv")), report)?;
    Ok(())
}
fn compare(source: &Path, target: &Path, patches: &[Patch]) -> Result<()> {
    let mut source = File::open(source)?;
    let mut target = File::open(target)?;
    let bytes = source.metadata()?.len();
    if target.metadata()?.len() != bytes {
        return Err(Error::new("storage-readback", "image size changed"));
    }
    let mut expected = vec![0; 1024 * 1024];
    let mut actual = vec![0; expected.len()];
    let mut offset = 0;
    while offset < bytes {
        let n = (bytes - offset).min(expected.len() as u64) as usize;
        source.read_exact(&mut expected[..n])?;
        target.read_exact(&mut actual[..n])?;
        for p in patches {
            let from = offset.max(p.offset);
            let to = (offset + n as u64).min(p.offset + p.bytes.len() as u64);
            if from < to {
                expected[(from - offset) as usize..(to - offset) as usize].copy_from_slice(
                    &p.bytes[(from - p.offset) as usize..(to - p.offset) as usize],
                );
            }
        }
        if expected[..n] != actual[..n] {
            return Err(Error::new(
                "storage-readback",
                format!("unexpected changed bytes in chunk at {offset}"),
            ));
        }
        offset += n as u64;
    }
    Ok(())
}
pub(super) fn compare_unchanged(source: &Path, target: &Path) -> Result<()> {
    compare(source, target, &[])
}
fn crc(bytes: &[u8]) -> u32 {
    let mut n = !0u32;
    for b in bytes {
        n ^= u32::from(*b);
        for _ in 0..8 {
            n = (n >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(n & 1));
        }
    }
    !n
}
fn corrupt(source: &Path, mode: &str) -> Result<Vec<Patch>> {
    let mut input = File::open(source)?;
    let sectors = if mode == "t04-both-gpt" {
        vec![1, 2097151]
    } else if mode == "t04-conflict-gpt" {
        vec![2097151]
    } else {
        vec![1]
    };
    let mut patches = Vec::new();
    for sector in sectors {
        let mut bytes = vec![0; 512];
        input.seek(SeekFrom::Start(sector * 512))?;
        input.read_exact(&mut bytes)?;
        if mode == "t04-conflict-gpt" {
            bytes[56] ^= 1;
            bytes[16..20].fill(0);
            let checksum = crc(&bytes[..92]);
            bytes[16..20].copy_from_slice(&checksum.to_le_bytes());
        } else {
            bytes[16] ^= 1;
        }
        patches.push(Patch {
            offset: sector * 512,
            bytes,
        });
    }
    Ok(patches)
}
pub(super) fn execute(
    context: &Context,
    runner: &mut Runner,
    manifest: &Manifest,
    machine: &crate::vm::Machine,
    kernel: &Path,
    source: &Path,
) -> Result<()> {
    use crate::vm::{BootSpec, boot};
    let write_boot = boot_image::create(&context.root, runner, manifest, kernel, "t04-write")?;
    let data = clone_sparse(source, runner)?;
    let offset = (manifest.number("last_lba")? + 1) * 512 - 4096;
    let initial = vec![Patch {
        offset,
        bytes: vec![0xcc; 4096],
    }];
    apply_patches(&data, &initial, "initial")?;
    compare(source, &data, &initial)?;
    crate::digest(&data, runner)?;
    boot(
        runner,
        manifest,
        machine,
        BootSpec {
            boot_disk: &write_boot,
            data_disk: Some(&data),
            readonly: false,
            mode: "t04-write",
        },
    )?;
    // 独立宿主期望来自本用例规格，不调用 guest 生产代码，也不信任其成功串口。
    let mut payload = vec![0xcc; 4096];
    for (i, b) in payload[..1024].iter_mut().enumerate() {
        *b = ((i * 17 + 0x53) % 256) as u8;
    }
    for (i, b) in payload[1023..2023].iter_mut().enumerate() {
        *b = ((i * 29 + 7) % 256) as u8;
    }
    let expected = vec![Patch {
        offset,
        bytes: payload,
    }];
    compare(source, &data, &expected)?;
    fs::write(
        data.parent().unwrap().join("expected-payload.bin"),
        &expected[0].bytes,
    )?;
    runner.record(&format!(
        "storage_write={} offset={offset} bytes=4096 outside_unchanged=true flush=guest-confirmed",
        data.display()
    ))?;
    crate::digest(&data, runner)?;
    crate::digest(&write_boot, runner)?;
    let reopen = boot_image::create(&context.root, runner, manifest, kernel, "t04-reopen")?;
    boot(
        runner,
        manifest,
        machine,
        BootSpec {
            boot_disk: &reopen,
            data_disk: Some(&data),
            readonly: true,
            mode: "t04-reopen",
        },
    )?;
    compare(source, &data, &expected)?;
    crate::digest(&data, runner)?;
    println!("T04 write, flush, restart and full-disk independent comparison: verified");
    for mode in ["t04-primary-gpt", "t04-both-gpt", "t04-conflict-gpt"] {
        let boot_disk = boot_image::create(&context.root, runner, manifest, kernel, mode)?;
        let data = clone_sparse(source, runner)?;
        let patches = corrupt(source, mode)?;
        apply_patches(&data, &patches, "gpt-fault")?;
        compare(source, &data, &patches)?;
        crate::digest(&data, runner)?;
        boot(
            runner,
            manifest,
            machine,
            BootSpec {
                boot_disk: &boot_disk,
                data_disk: Some(&data),
                readonly: true,
                mode,
            },
        )?;
        compare(source, &data, &patches)?;
        crate::digest(&data, runner)?;
        runner.record(&format!(
            "storage_fault={mode} disk={} unchanged=true",
            data.display()
        ))?;
        println!("T04 {mode}: verified");
    }
    for mode in ["t04-timeout", "t04-no-flush"] {
        let disk = boot_image::create(&context.root, runner, manifest, kernel, mode)?;
        boot(
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
        crate::digest(&disk, runner)?;
        println!("T04 {mode}: verified stop");
    }
    Ok(())
}

pub fn validate(output: &str, mode: &str) -> Result<()> {
    let lines = output
        .lines()
        .filter(|l| l.starts_with("RR:T04:"))
        .collect::<Vec<_>>();
    let count = |prefix: &str| lines.iter().filter(|l| l.starts_with(prefix)).count();
    if lines.first() != Some(&"RR:T04:ENTER") || count("RR:T04:ENTER") != 1 {
        return Err(Error::new("storage-sequence", "missing/duplicate entry"));
    }
    if !mode.is_empty()
        && (lines.get(1) != Some(&format!("RR:T04:CASE name={mode}").as_str())
            || count("RR:T04:CASE") != 1)
    {
        return Err(Error::new("storage-case", "missing case identity"));
    }
    let fault = match mode {
        "t04-both-gpt" => Some("GPT_INVALID"),
        "t04-conflict-gpt" => Some("GPT_CONFLICT"),
        "t04-timeout" => {
            let wait = output
                .lines()
                .find(|l| l.starts_with("RR:T06:WAIT "))
                .ok_or_else(|| Error::new("storage-timeout", "missing wait cause"))?;
            let f = fields(wait)?;
            let code = match f.get("reason") {
                Some(&"DEADLINE") => "BLOCK_TIMEOUT",
                Some(&"POLL_BUDGET_EXHAUSTED") => "BLOCK_POLL_BUDGET",
                _ => return Err(Error::new("storage-timeout", "unexpected wait cause")),
            };
            crate::vm_runtime::validate_wait(output, code)?;
            Some(code)
        }
        "t04-no-flush" => Some("BLOCK_FLUSH_UNSUPPORTED"),
        _ => None,
    };
    if let Some(code) = fault {
        if count("RR:T04:READY") != 0
            || count("RR:T04:FAIL") != 1
            || !lines
                .last()
                .is_some_and(|l| l.starts_with(&format!("RR:T04:FAIL code={code} stage=")))
        {
            return Err(Error::new("storage-fault", "wrong terminal failure"));
        }
        if mode == "t04-timeout" {
            let proof = lines
                .iter()
                .find(|l| l.starts_with("RR:T04:QUARANTINE "))
                .ok_or_else(|| Error::new("storage-timeout", "missing retained DMA evidence"))?;
            let f = fields(proof)?;
            if count("RR:T04:QUARANTINE ") != 1
                || count("RR:T04:STOP") != 0
                || number(&f, "shares")? < 2
                || number(&f, "allocations")? == 0
                || number(&f, "release_rejected")? != 1
                || number(&f, "allocation_rejected")? != 1
                || number(&f, "reused")? != 0
            {
                return Err(Error::new(
                    "storage-timeout",
                    "resources released while request incomplete",
                ));
            }
        }
        return Ok(());
    }
    let label = if mode.is_empty() { "probe" } else { mode };
    if count("RR:T04:FAIL") != 0
        || count("RR:T04:READY") != 1
        || lines.last() != Some(&format!("RR:T04:READY mode={label}").as_str())
    {
        return Err(Error::new("storage-sequence", "unexpected terminal"));
    }
    let one = |prefix: &str| -> Result<_> {
        if count(prefix) != 1 {
            return Err(Error::new(
                "storage-facts",
                format!("expected one {prefix}"),
            ));
        }
        fields(lines.iter().find(|l| l.starts_with(prefix)).unwrap())
    };
    let device = one("RR:T04:DEVICE ")?;
    if number(&device, "sectors")? != 2097152
        || number(&device, "block_size")? != 512
        || number(&device, "flush")? != 1
        || number(&device, "queue")? != 16
        || number(&device, "readonly")? != u64::from(mode != "t04-write")
    {
        return Err(Error::new("storage-device", "unexpected device facts"));
    }
    let gpt = one("RR:T04:GPT ")?;
    if number(&gpt, "data_start")? != 264192
        || number(&gpt, "data_sectors")? != 1832927
        || number(&gpt, "esp_start")? != 2048
        || number(&gpt, "esp_sectors")? != 262144
        || number(&gpt, "degraded")? != u64::from(mode == "t04-primary-gpt")
        || number(&gpt, "readonly")? != u64::from(mode != "t04-write")
    {
        return Err(Error::new("storage-gpt", "unexpected partition facts"));
    }
    let protection = one("RR:T04:PROTECTION ")?;
    if number(&protection, "bounds_rejected")? != 6
        || number(&protection, "submitted")? != 0
        || number(&protection, "readonly_rejected")? != u64::from(mode != "t04-write")
    {
        return Err(Error::new(
            "storage-protection",
            "rejected request reached device",
        ));
    }
    let stop = one("RR:T04:STOP ")?;
    // 无效主表头必须阻止读取其数组：MBR + 两个表头 + 备用数组 + DATA = 5。
    let expected_requests = match mode {
        "t04-primary-gpt" => 5,
        "t04-write" => 30,
        "t04-reopen" => 13,
        _ => 6,
    };
    if stop.get("reset") != Some(&"confirmed")
        || number(&stop, "bus_master")? != 0
        || number(&stop, "allocations")? != 0
        || number(&stop, "shared")? != 0
        || number(&stop, "requests")? != expected_requests
        || number(&stop, "flushes")? != u64::from(mode == "t04-write")
    {
        return Err(Error::new(
            "storage-stop",
            "missing reset / release / flush evidence",
        ));
    }
    if matches!(mode, "t04-write" | "t04-reopen") {
        let payload = one("RR:T04:PAYLOAD ")?;
        if payload.get("mode") != Some(&mode)
            || number(&payload, "length")? != 4096
            || number(&payload, "offset")? != 1832927 * 512 - 4096
            || number(&payload, "verified")? != 1
        {
            return Err(Error::new(
                "storage-payload",
                "missing payload verification",
            ));
        }
    }
    if mode == "t04-write"
        && !lines.contains(&"RR:T04:DEVICE_ERROR kind=unsupported source=real-response recovered=1")
    {
        return Err(Error::new(
            "storage-response",
            "missing real negative response",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn degraded_gpt_skips_invalid_primary_array() {
        let trace = r#"RR:T04:ENTER
RR:T04:CASE name=t04-primary-gpt
RR:T04:DEVICE sectors=2097152 block_size=512 readonly=1 flush=1 queue=16
RR:T04:GPT data_start=264192 data_sectors=1832927 esp_start=2048 esp_sectors=262144 degraded=1 readonly=1 disk_guid=11111111-2222-4333-8444-555555555555 data_guid=12345678-1234-4234-8234-123456789abc
RR:T04:GPT_DIAGNOSTIC primary=Some(HeaderCrc) backup=None
RR:T04:PROTECTION bounds_rejected=6 readonly_rejected=1 submitted=0
RR:T04:READ length=512 checksum=0x8a9e5f705eb1f9f5
RR:T04:STOP reset=confirmed bus_master=0 allocations=0 shared=0 requests=5 flushes=0
RR:T04:READY mode=t04-primary-gpt
"#;
        assert!(validate(trace, "t04-primary-gpt").is_ok());
        assert!(
            validate(
                &trace.replace("requests=5", "requests=6"),
                "t04-primary-gpt"
            )
            .is_err()
        );
        assert!(
            validate(
                &trace.replace("allocations=0", "allocations=1"),
                "t04-primary-gpt"
            )
            .is_err()
        );
    }
    #[test]
    fn timeout_rejects_premature_release() {
        let text = "RR:T06:WAIT reason=DEADLINE elapsed_ns=2000000000 stage=request\nRR:T04:ENTER\nRR:T04:CASE name=t04-timeout\nRR:T04:QUARANTINE shares=3 allocations=3 release_rejected=1 allocation_rejected=1 reused=0\nRR:T04:FAIL code=BLOCK_TIMEOUT stage=Read\n";
        assert!(validate(text, "t04-timeout").is_ok());
        assert!(
            validate(
                &text.replace("allocation_rejected=1", "allocation_rejected=0"),
                "t04-timeout"
            )
            .is_err()
        );
        assert!(
            validate(
                &format!("{text}RR:T04:READY mode=t04-timeout\n"),
                "t04-timeout"
            )
            .is_err()
        );
    }
}
