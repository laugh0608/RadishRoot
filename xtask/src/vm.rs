use crate::{
    boot_image::{self, Manifest},
    build,
    cli::Action,
    config::Context,
    error::{Error, Result},
    process::{self, Runner},
};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

fn executable() -> Result<PathBuf> {
    for directory in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        let candidate = directory.join("qemu-system-x86_64");
        if candidate.is_file() {
            return Ok(candidate.canonicalize()?);
        }
    }
    Err(Error::new(
        "qemu-missing",
        "install the authorized exact QEMU before running",
    ))
}

pub(super) struct Machine {
    pub qemu: PathBuf,
    pub code: PathBuf,
    pub vars: PathBuf,
    pub host_view: Option<PathBuf>,
}
pub(super) struct BootSpec<'a> {
    pub boot_disk: &'a Path,
    pub data_disk: Option<&'a Path>,
    pub readonly: bool,
    pub mode: &'a str,
}

pub fn execute(context: &Context, runner: &mut Runner, action: &Action) -> Result<()> {
    let manifest = Manifest::load(&context.root)?;
    let selected = if let Action::TestGraphicsCases(cases) = action {
        Some(cases)
    } else {
        None
    };
    let kernel = build::kernel(context, runner, true, true, selected.is_some())?;
    let first_mode = selected.map_or("", |cases| cases[0]);
    let disk = boot_image::create(&context.root, runner, &manifest, &kernel, first_mode)?;
    if *action == Action::Image {
        return Ok(());
    }
    let qemu = executable()?;
    let version = runner.run(Command::new(&qemu).arg("--version"))?;
    let expected = format!("QEMU emulator version {}", manifest.get("qemu_version")?);
    if version.lines().next() != Some(expected.as_str()) {
        return Err(Error::new("qemu-version", version));
    }
    let share = qemu
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| Error::new("qemu-path", "missing prefix"))?
        .join("share/qemu");
    let code = share.join(manifest.get("firmware_code")?);
    let vars = share.join(manifest.get("firmware_vars")?);
    manifest.hash(runner, &code, "firmware_code_sha256")?;
    manifest.hash(runner, &vars, "firmware_vars_sha256")?;
    let host_view = if matches!(action, Action::TestStorageVm | Action::TestFatVm) {
        None
    } else {
        Some(crate::ui::build(context, runner, false)?)
    };
    let machine = Machine {
        qemu,
        code,
        vars,
        host_view,
    };
    if let Some(cases) = selected {
        for (index, mode) in cases.iter().enumerate() {
            let selected_disk = if index == 0 {
                disk.clone()
            } else {
                boot_image::create(&context.root, runner, &manifest, &kernel, mode)?
            };
            boot(
                runner,
                &manifest,
                &machine,
                BootSpec {
                    boot_disk: &selected_disk,
                    data_disk: None,
                    readonly: true,
                    mode,
                },
            )?;
            println!("graphics {mode}: verified");
        }
        return runner.record(&format!(
            "selected T07 cases complete: {}; not the full suite",
            cases.join(",")
        ));
    }
    let repetitions = match action {
        Action::TestVm => 10,
        Action::TestStorageVm | Action::TestFatVm => 0,
        _ => 1,
    };
    for number in 1..=repetitions {
        boot_configured(
            runner,
            &manifest,
            &machine,
            BootSpec {
                boot_disk: &disk,
                data_disk: None,
                readonly: true,
                mode: "",
            },
            number == 1 && matches!(action, Action::TestVm | Action::TestGraphicsVm),
            *action == Action::RunGui,
        )?;
        println!("normal boot {number}/{repetitions}: verified");
    }
    crate::digest(&disk, runner)?;
    if matches!(
        action,
        Action::TestVm
            | Action::TestStorageVm
            | Action::TestFatVm
            | Action::TestRuntimeVm
            | Action::TestGraphicsVm
    ) {
        let kernel = build::kernel(context, runner, true, true, true)?;
        for fault in [
            "unsupported-protocol",
            "missing-response",
            "invalid-framebuffer",
            "panic",
            "t03-invalid-range",
            "t03-bad-capability",
            "t03-dma-timeout",
        ] {
            if matches!(
                action,
                Action::TestStorageVm
                    | Action::TestFatVm
                    | Action::TestRuntimeVm
                    | Action::TestGraphicsVm
            ) {
                break;
            }
            let disk = boot_image::create(&context.root, runner, &manifest, &kernel, fault)?;
            boot(
                runner,
                &manifest,
                &machine,
                BootSpec {
                    boot_disk: &disk,
                    data_disk: None,
                    readonly: true,
                    mode: fault,
                },
            )?;
            println!("fault {fault}: verified stop");
        }
        if matches!(action, Action::TestVm | Action::TestStorageVm) {
            crate::vm_storage::execute(context, runner, &manifest, &machine, &kernel, &disk)?;
        }
        if matches!(action, Action::TestVm | Action::TestFatVm) {
            crate::vm_fat::execute(context, runner, &manifest, &machine, &kernel, &disk)?;
        }
        if matches!(action, Action::TestVm | Action::TestRuntimeVm) {
            crate::vm_runtime::execute(context, runner, &manifest, &machine, &kernel, &disk)?;
        }
        if matches!(action, Action::TestVm | Action::TestGraphicsVm) {
            crate::vm_graphics::execute(context, runner, &manifest, &machine, &kernel)?;
        }
    }
    runner.record(if *action == Action::TestGraphicsVm {
        "T07 nine graphics cases complete; standalone T02-T06 regressions not run; host termination is not guest shutdown"
    } else if *action == Action::TestRuntimeVm {
        "T06 runtime cases complete; standalone T02-T05 regressions not run; host termination is not guest shutdown"
    } else if *action == Action::TestFatVm {
        "T05 FAT cases complete; T02/T03/T04 standalone regressions not run; host termination is not guest shutdown"
    } else if *action == Action::TestStorageVm {
        "T04 storage cases complete; T02/T03 standalone regressions not run; host termination is not guest shutdown"
    } else if *action == Action::TestVm {
        "T02/T03/T04/T05/T06/T07 complete: 47 VM starts, actual input/screens and independent storage checks; host termination is not guest shutdown"
    } else {
        "one normal read-only boot complete; standalone fault suites not run; no guest shutdown claim"
    })
}
pub(super) fn boot(
    runner: &mut Runner,
    manifest: &Manifest,
    machine: &Machine,
    spec: BootSpec<'_>,
) -> Result<String> {
    boot_configured(runner, manifest, machine, spec, false, false)
}
fn boot_configured(
    runner: &mut Runner,
    manifest: &Manifest,
    machine: &Machine,
    spec: BootSpec<'_>,
    full: bool,
    gui: bool,
) -> Result<String> {
    let fault = spec.mode;
    let disk = spec.data_disk.unwrap_or(spec.boot_disk);
    let qemu = &machine.qemu;
    let code = &machine.code;
    let vars = &machine.vars;
    let directory = process::unique_directory(&runner.dir, "boot")?;
    let writable_vars = directory.join("vars.fd");
    fs::copy(vars, &writable_vars)?;
    let serial = directory.join("serial.log");
    fs::write(&serial, "")?;
    let mut command = Command::new(qemu);
    command
        .args([
            "-machine",
            &format!(
                "{},hpet={}",
                manifest.get("machine")?,
                if fault == "t06-no-hpet" { "off" } else { "on" }
            ),
            "-accel",
            "tcg",
            "-cpu",
            "qemu64",
            "-smp",
            manifest.get("cpus")?,
            "-m",
            manifest.get("memory_mib")?,
            "-nodefaults",
            "-display",
            if gui { "cocoa" } else { "none" },
            "-monitor",
            "none",
            "-nic",
            "none",
            "-no-reboot",
            "-no-shutdown",
            "-device",
            "VGA",
            "-serial",
        ])
        .arg(format!("file:{}", serial.display()))
        .arg("-drive")
        .arg(format!(
            "if=pflash,format=raw,unit=0,readonly=on,file={}",
            code.display()
        ))
        .arg("-drive")
        .arg(format!(
            "if=pflash,format=raw,unit=1,file={}",
            writable_vars.display()
        ))
        .arg("-drive")
        .arg(format!(
            "if=none,id=os,format=raw,cache=writeback,aio=threads,readonly={},file={}",
            if spec.readonly { "on" } else { "off" },
            disk.display()
        ))
        .args([
            "-device",
            "virtio-blk-pci,drive=os,disable-legacy=on",
            "-boot",
            "strict=on",
        ]);
    if spec.data_disk.is_some() {
        command
            .arg("-drive")
            .arg(format!(
                "if=none,id=boot,format=raw,readonly=on,file={}",
                spec.boot_disk.display()
            ))
            .args([
                "-device",
                "virtio-scsi-pci,id=bootctl,disable-legacy=on",
                "-device",
                "scsi-hd,drive=boot,bus=bootctl.0,bootindex=1",
            ]);
    }
    runner.record(&format!(
        "storage_boot={} data={} readonly={} cache=writeback snapshot=off",
        spec.boot_disk.display(),
        disk.display(),
        spec.readonly
    ))?;
    let graphics = fault.is_empty() || fault.starts_with("t07-");
    let socket_dir = if graphics {
        Some(process::unique_directory(
            Path::new("/private/tmp"),
            "rr-qmp",
        )?)
    } else {
        None
    };
    let socket = socket_dir.as_ref().map(|p| p.join("qmp.sock"));
    if let Some(socket) = &socket {
        command
            .arg("-qmp")
            .arg(format!("unix:{},server=on,wait=off", socket.display()));
        runner.record(&format!(
            "qmp_private_socket={} gui={gui} global_grab=off",
            socket.display()
        ))?;
    }
    let previous = runner.timeout;
    runner.timeout = if gui {
        Duration::from_secs(600)
    } else {
        previous.min(Duration::from_secs(
            manifest.number("boot_timeout_seconds")?,
        ))
    };
    let mut session = socket.as_ref().map(|socket| {
        crate::vm_graphics::Session::new(&serial, socket, &directory, fault, full, gui)
    });
    let output = if let Some(session) = &mut session {
        runner.interactive(&mut command, &serial, &mut || session.poll())
    } else {
        runner.boot(
            &mut command,
            &serial,
            fault.is_empty() || fault.starts_with("t06-"),
        )
    };
    runner.timeout = previous;
    if let Some(socket_dir) = socket_dir {
        let path = socket_dir.join("qmp.sock");
        if path.exists() {
            fs::remove_file(&path)?;
        }
        fs::remove_dir(&socket_dir)?;
        runner.record(&format!("qmp_private_socket_removed={}", path.display()))?;
    }
    let output = output?;
    validate_size(
        &output,
        if fault.starts_with("t03-")
            || fault.starts_with("t04-")
            || fault.starts_with("t05-")
            || fault.starts_with("t06-")
            || fault.starts_with("t07-")
        {
            ""
        } else {
            fault
        },
        if fault == "t07-resolution" {
            (800, 600)
        } else {
            (1024, 768)
        },
    )?;
    if fault.is_empty()
        || fault.starts_with("t03-")
        || fault.starts_with("t04-")
        || fault.starts_with("t05-")
        || fault.starts_with("t06-")
        || fault.starts_with("t07-")
    {
        crate::vm_platform::validate(
            &output,
            if fault.starts_with("t04-")
                || fault.starts_with("t05-")
                || fault.starts_with("t06-")
                || fault.starts_with("t07-")
            {
                ""
            } else {
                fault
            },
        )?;
    }
    if fault.is_empty() || fault.starts_with("t04-") || fault.starts_with("t07-") {
        crate::vm_storage::validate(&output, if fault.starts_with("t07-") { "" } else { fault })?;
    }
    if fault.starts_with("t05-") {
        crate::vm_fat::validate(&output, fault)?;
    }
    if fault.is_empty() || fault.starts_with("t06-") || fault.starts_with("t07-") {
        crate::vm_runtime::validate(&output, if fault.starts_with("t07-") { "" } else { fault })?;
    }
    if let Some(session) = session {
        crate::vm_graphics::validate_recording(&output, fault, full, gui)?;
        crate::vm_graphics::negative_controls(&output, fault, full, gui, &directory)?;
        let host = machine
            .host_view
            .as_ref()
            .ok_or_else(|| Error::new("host-view", "missing host comparator"))?;
        crate::screen::verify(runner, host, &directory, &session.snapshots)?;
    }
    runner.record(&format!(
        "boot={} case={fault:?} validation=pass",
        directory.display()
    ))?;
    Ok(output)
}
pub(super) fn fields(line: &str) -> Result<BTreeMap<&str, &str>> {
    let mut result = BTreeMap::new();
    for field in line.split_whitespace().skip(1) {
        let (key, value) = field
            .split_once('=')
            .ok_or_else(|| Error::new("serial-fields", "missing equals"))?;
        if result.insert(key, value).is_some() {
            return Err(Error::new("serial-fields", "duplicate key"));
        }
    }
    Ok(result)
}
pub(super) fn number(fields: &BTreeMap<&str, &str>, key: &str) -> Result<u64> {
    let text = fields
        .get(key)
        .ok_or_else(|| Error::new("serial-fields", format!("missing {key}")))?;
    let value = if let Some(hex) = text.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)
    } else {
        text.parse()
    };
    value.map_err(|_| Error::new("serial-fields", format!("invalid {key}")))
}
#[cfg(test)]
pub fn validate(output: &str, fault: &str) -> Result<()> {
    validate_size(output, fault, (1024, 768))
}
fn validate_size(output: &str, fault: &str, size: (u64, u64)) -> Result<()> {
    let lines = output
        .lines()
        .filter(|line| line.starts_with("RR:T02:"))
        .collect::<Vec<_>>();
    let count = |prefix: &str| lines.iter().filter(|s| s.starts_with(prefix)).count();
    if lines.first() != Some(&"RR:T02:ENTER") || count("RR:T02:ENTER") != 1 {
        return Err(Error::new("serial-sequence", "missing or duplicate entry"));
    }
    if !fault.is_empty() {
        let code = match fault {
            "unsupported-protocol" => "BOOT_PROTOCOL",
            "missing-response" => "BOOT_RESPONSE_MISSING",
            "invalid-framebuffer" => "BOOT_FB_GEOMETRY",
            "panic" => "BOOT_PANIC",
            _ => return Err(Error::new("serial-case", "unknown case")),
        };
        let injection = format!("RR:T02:INJECT case={fault}");
        let terminal = format!("RR:T02:FAIL code={code} stage=");
        if lines.get(1) != Some(&injection.as_str())
            || count("RR:T02:INJECT") != 1
            || count("RR:T02:READY") != 0
            || count("RR:T02:FAIL") != 1
            || !lines.last().is_some_and(|line| line.starts_with(&terminal))
        {
            return Err(Error::new(
                "serial-fault",
                format!("unexpected fault sequence for {fault}"),
            ));
        }
        return Ok(());
    }
    if count("RR:T02:FAIL") != 0
        || count("RR:T02:INJECT") != 0
        || count("RR:T02:READY") != 1
        || lines.last() != Some(&"RR:T02:READY revision=6")
    {
        return Err(Error::new(
            "serial-sequence",
            "unexpected normal terminal sequence",
        ));
    }
    let map = lines
        .iter()
        .filter(|s| s.starts_with("RR:T02:MEM "))
        .map(|s| fields(s))
        .collect::<Result<Vec<_>>>()?;
    if map.is_empty() || map.len() > 256 || lines.len() != map.len() + 5 {
        return Err(Error::new("serial-map", "wrong record count"));
    }
    let mut previous = 0;
    let mut usable = 0;
    for (i, entry) in map.iter().enumerate() {
        let base = number(entry, "base")?;
        let length = number(entry, "length")?;
        if lines[i + 1]
            != lines
                .iter()
                .find(|s| s.starts_with(&format!("RR:T02:MEM index={i} ")))
                .copied()
                .unwrap_or("")
            || number(entry, "index")? != i as u64
            || length == 0
            || base < previous
        {
            return Err(Error::new(
                "serial-map",
                "unordered or overlapping memory records",
            ));
        }
        previous = base
            .checked_add(length)
            .ok_or_else(|| Error::new("serial-map", "range overflow"))?;
        if number(entry, "kind")? == 0 {
            usable += length;
        }
    }
    let one = |prefix: &str| -> Result<BTreeMap<&str, &str>> {
        if count(prefix) != 1 {
            return Err(Error::new(
                "serial-fields",
                format!("expected one {prefix}"),
            ));
        }
        fields(lines.iter().find(|s| s.starts_with(prefix)).unwrap())
    };
    let memory = one("RR:T02:MEMORY ")?;
    if number(&memory, "entries")? != map.len() as u64
        || number(&memory, "usable")? != usable
        || usable == 0
        || memory.get("reclaim") != Some(&"none")
    {
        return Err(Error::new("serial-memory", "summary differs from real map"));
    }
    let kernel = one("RR:T02:KERNEL ")?;
    let fb = one("RR:T02:FB ")?;
    if number(&kernel, "virtual")? != 0xffffffff80000000
        || number(&fb, "width")? != size.0
        || number(&fb, "height")? != size.1
        || number(&fb, "bpp")? != 32
        || number(&fb, "pitch")? < size.0 * 4
    {
        return Err(Error::new(
            "serial-layout",
            "unexpected kernel/framebuffer layout",
        ));
    }
    let fb_length = number(&fb, "pitch")?
        .checked_mul(size.1)
        .ok_or_else(|| Error::new("serial-layout", "framebuffer overflow"))?;
    for (object, length, kind) in [
        (&kernel, number(&kernel, "length")?, 6),
        (&fb, fb_length, 7),
    ] {
        let base = number(object, "physical")?;
        let end = base
            .checked_add(length)
            .filter(|_| length > 0)
            .ok_or_else(|| Error::new("serial-layout", "object overflow"))?;
        let covered = map.iter().any(|r| {
            number(r, "kind").ok() == Some(kind)
                && number(r, "base").is_ok_and(|b| b <= base)
                && number(r, "base")
                    .ok()
                    .zip(number(r, "length").ok())
                    .and_then(|(b, l)| b.checked_add(l))
                    .is_some_and(|limit| end <= limit)
        });
        if !covered {
            return Err(Error::new("serial-layout", "object not in reserved region"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const GOOD: &str = "firmware output\nRR:T02:ENTER\nRR:T02:MEM index=0 base=0x1000 length=0x1000 kind=0\nRR:T02:MEM index=1 base=0x2000 length=0x1000 kind=6\nRR:T02:MEM index=2 base=0xe0000000 length=0x300000 kind=7\nRR:T02:KERNEL physical=0x2000 length=0x1000 virtual=0xffffffff80000000\nRR:T02:FB physical=0xe0000000 width=1024 height=768 pitch=4096 bpp=32\nRR:T02:MEMORY entries=3 usable=4096 reclaim=none\nRR:T02:READY revision=6\n";
    #[test]
    fn validates_facts_and_rejects_false_success() {
        assert!(validate(GOOD, "").is_ok());
        for bad in [
            GOOD.replace("usable=4096", "usable=8192"),
            GOOD.replace("kind=6", "kind=0"),
            GOOD.replace("index=1", "index=0"),
            GOOD.replace("pitch=4096", "pitch=1"),
            format!("{GOOD}RR:T02:READY revision=6\n"),
            format!("{GOOD}RR:T02:FAIL code=BOOT_PANIC stage=panic\n"),
        ] {
            assert!(validate(&bad, "").is_err());
        }
    }
    #[test]
    fn expected_fault_requires_matching_injection_and_terminal() {
        let good = "RR:T02:ENTER\nRR:T02:INJECT case=missing-response\nRR:T02:FAIL code=BOOT_RESPONSE_MISSING stage=boot-validation\n";
        assert!(validate(good, "missing-response").is_ok());
        assert!(validate(good, "panic").is_err());
        assert!(validate(good, "").is_err());
        assert!(
            validate(
                &format!("{good}RR:T02:READY revision=6\n"),
                "missing-response"
            )
            .is_err()
        );
    }
}
