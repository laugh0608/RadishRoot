use crate::{
    error::{Error, Result},
    process::{self, Runner},
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub struct Manifest(BTreeMap<String, String>);
impl Manifest {
    pub fn load(root: &Path) -> Result<Self> {
        let source = fs::read_to_string(root.join("tools/boot/manifest.tsv"))?;
        if source != include_str!("../../tools/boot/manifest.tsv") {
            return Err(Error::new(
                "boot-manifest",
                "manifest changed since xtask compilation",
            ));
        }
        let mut values = BTreeMap::new();
        for line in source
            .lines()
            .filter(|s| !s.is_empty() && !s.starts_with('#'))
        {
            let (key, value) = line
                .split_once('\t')
                .ok_or_else(|| Error::new("boot-manifest", "expected TSV"))?;
            if values.insert(key.into(), value.into()).is_some() {
                return Err(Error::new("boot-manifest", "duplicate key"));
            }
        }
        let m = Self(values);
        for key in [
            "memory_mib",
            "cpus",
            "disk_bytes",
            "esp_start",
            "esp_sectors",
            "data_start",
            "last_lba",
            "boot_timeout_seconds",
        ] {
            m.number(key)?;
        }
        if m.number("disk_bytes")? != 1024 * 1024 * 1024
            || m.number("esp_start")? != 2048
            || m.number("esp_sectors")? != 262144
            || m.number("data_start")? != 264192
            || m.number("last_lba")? != 2097118
            || m.number("cpus")? != 1
            || m.number("memory_mib")? != 512
            || m.number("boot_timeout_seconds")? != 60
        {
            return Err(Error::new(
                "boot-manifest",
                "T02 authorized resource/layout boundary differs",
            ));
        }
        Ok(m)
    }
    pub fn get(&self, key: &str) -> Result<&str> {
        self.0
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| Error::new("boot-manifest", format!("missing {key}")))
    }
    pub fn number(&self, key: &str) -> Result<u64> {
        self.get(key)?
            .parse()
            .map_err(|_| Error::new("boot-manifest", format!("invalid integer {key}")))
    }
    pub fn hash(&self, runner: &mut Runner, path: &Path, key: &str) -> Result<()> {
        if !fs::symlink_metadata(path)?.is_file() {
            return Err(Error::new("boot-asset", "expected regular file"));
        }
        let output = runner.run(Command::new("shasum").args(["-a", "256"]).arg(path))?;
        if output.split_whitespace().next() != Some(self.get(key)?) {
            return Err(Error::new(
                "boot-asset-hash",
                format!("{key}: {}", path.display()),
            ));
        }
        runner.record(output.trim())
    }
}

pub fn create(
    root: &Path,
    runner: &mut Runner,
    manifest: &Manifest,
    kernel: &Path,
    fault: &str,
) -> Result<PathBuf> {
    if ![
        "",
        "unsupported-protocol",
        "missing-response",
        "invalid-framebuffer",
        "panic",
        "t03-invalid-range",
        "t03-bad-capability",
        "t03-dma-timeout",
    ]
    .contains(&fault)
    {
        return Err(Error::new("boot-case", "unknown fault"));
    }
    let directory = process::unique_directory(&runner.dir, "image")?;
    let inputs = directory.join("inputs");
    let output = directory.join("output");
    process::directory(&inputs)?;
    process::directory(&output)?;
    for (name, key) in [
        ("BOOTX64.EFI", "limine_efi_sha256"),
        ("LICENSE", "limine_license_sha256"),
        ("3RDPARTY.md", "limine_notices_sha256"),
    ] {
        let source = root.join("work/t02-assets").join(name);
        manifest.hash(runner, &source, key)?;
        fs::copy(source, inputs.join(name))?;
    }
    process::directory(&inputs.join("crate-licenses"))?;
    let mut licenses =
        fs::read_dir(root.join("tools/boot/licenses"))?.collect::<std::io::Result<Vec<_>>>()?;
    licenses.sort_by_key(|entry| entry.file_name());
    if licenses.is_empty() || licenses.len() > 32 {
        return Err(Error::new("licenses", "invalid license count"));
    }
    for entry in licenses {
        if !entry.file_type()?.is_file()
            || entry.path().extension().is_none_or(|s| s != "txt")
            || entry.metadata()?.len() > 128 * 1024
        {
            return Err(Error::new("licenses", "expected bounded text license"));
        }
        fs::copy(
            entry.path(),
            inputs.join("crate-licenses").join(entry.file_name()),
        )?;
    }
    fs::copy(kernel, inputs.join("kernel.elf"))?;
    fs::copy(
        root.join("tools/boot/build-disk.sh"),
        inputs.join("build-disk.sh"),
    )?;
    fs::write(
        inputs.join("limine.conf"),
        format!(
            "timeout: 0\nserial: yes\n\n/RadishRoot\n    protocol: limine\n    path: boot():/kernel.elf\n    resolution: 1024x768x32\n    cmdline: {fault}\n"
        ),
    )?;
    let name = format!(
        "radishroot-{}",
        directory.file_name().unwrap().to_string_lossy()
    );
    let image = manifest.get("debian_image")?;
    let identity = runner
        .run(Command::new("docker").args(["image", "inspect", "--format", "{{.Id}}", image]))?;
    if identity.trim() != image {
        return Err(Error::new("tool-image", "image identity mismatch"));
    }
    let mut command = Command::new("docker");
    command
        .args([
            "run",
            "--rm",
            "--pull=never",
            "--name",
            &name,
            "--network=none",
            "--cap-drop=ALL",
            "--read-only",
            "--tmpfs",
            "/tmp:rw,noexec,nosuid,size=16m",
            "--mount",
        ])
        .arg(format!(
            "type=bind,src={},dst=/inputs,readonly",
            inputs.display()
        ))
        .arg("--mount")
        .arg(format!("type=bind,src={},dst=/output", output.display()))
        .args([image, "/bin/sh", "/inputs/build-disk.sh"]);
    for key in [
        "disk_bytes",
        "esp_start",
        "esp_sectors",
        "data_start",
        "last_lba",
    ] {
        command.arg(manifest.number(key)?.to_string());
    }
    if let Err(error) = runner.run(&mut command) {
        // Docker daemon children are not in the CLI group. Cleanup only this task's unique name.
        let cleanup = runner.run(Command::new("docker").args(["rm", "-f", &name]));
        runner.record(&format!("container_cleanup={cleanup:?}"))?;
        return Err(error);
    }
    process::reject_output_symlinks(&directory)?;
    let disk = output.join("disk.raw");
    if fs::metadata(&disk)?.len() != manifest.number("disk_bytes")? {
        return Err(Error::new("disk-size", "unexpected disk size"));
    }
    crate::digest(&disk, runner)?;
    println!("image={}", disk.display());
    Ok(disk)
}
