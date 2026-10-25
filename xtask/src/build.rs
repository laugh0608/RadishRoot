use crate::{
    config::{Context, TARGET},
    elf,
    error::{Error, Result},
    process::{self, Runner},
};
use std::path::PathBuf;
pub fn kernel(
    context: &Context,
    runner: &mut Runner,
    release: bool,
    fresh: bool,
    faults: bool,
) -> Result<PathBuf> {
    let root = &context.root;
    let target_dir = if fresh {
        process::unique_directory(&runner.dir, "build")?
    } else {
        let path = root.join("target/kernel");
        process::directory(&path)?;
        path
    };
    process::reject_output_symlinks(&target_dir)?;
    let mut cmd = context.cargo(&[
        "build",
        "--locked",
        "--offline",
        "-p",
        "radishroot-kernel",
        "--bin",
        "radishroot-kernel",
        "--target",
        TARGET,
    ]);
    cmd.arg("--target-dir")
        .arg(&target_dir)
        .env("CARGO_INCREMENTAL", "0");
    if release {
        cmd.arg("--release");
    }
    if faults {
        cmd.args(["--features", "boot-test"]);
    }
    runner.run(&mut cmd)?;
    let artifact = target_dir
        .join(TARGET)
        .join(if release { "release" } else { "debug" })
        .join("radishroot-kernel");
    let resolved = artifact.canonicalize()?;
    if !resolved.starts_with(target_dir.canonicalize()?) {
        return Err(Error::new(
            "output-path",
            "artifact escapes output directory",
        ));
    }
    let layout = elf::read(&artifact)?;
    crate::digest(&artifact, runner)?;
    runner.record(&format!(
        "artifact={} entry={:#x} loads={} fresh={fresh} profile={}",
        artifact.display(),
        layout.entry,
        layout.loads,
        if release { "release" } else { "dev" }
    ))?;
    Ok(artifact)
}
