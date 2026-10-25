mod cli;
mod config;
mod elf;
mod error;
mod process;

use cli::{Action, Options};
use config::{Context, TARGET};
use error::{Error, Result};
use process::Runner;
use std::{env, fs, path::Path, process::Command, time::Duration};

fn main() {
    let result = cli::parse(&env::args().skip(1).collect::<Vec<_>>()).and_then(execute);
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(error.exit);
    }
}
fn execute(options: Options) -> Result<()> {
    if options.action == Action::Help {
        println!(
            "cargo +<project channel> xtask check | build --profile dev|release [--fresh] | test --host\nOptional: --timeout-secs 1..3600 (default 120 per child).\nOutputs: target/t01-*/; exit: 0 success, 1 failure, 2 usage, 124 timeout.\nT01 produces a static ELF only; no boot/run/image implementation."
        );
        return Ok(());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .canonicalize()?;
    process::directory(&root.join("target"))?;
    let dir = process::unique_directory(&root.join("target"), "t01")?;
    println!("run={}", dir.display());
    let mut runner = Runner::new(dir.clone(), Duration::from_secs(options.timeout))?;
    let result = perform(&root, &options, &mut runner);
    runner.record(&match &result {
        Ok(()) => "result=success (T01 static/host only)".into(),
        Err(e) => format!(
            "result=failure kind={} exit={} detail={}",
            e.kind, e.exit, e.detail
        ),
    })?;
    result
}
fn perform(root: &Path, options: &Options, runner: &mut Runner) -> Result<()> {
    let context = Context::preflight(root, runner)?;
    runner.run(
        Command::new("git")
            .current_dir(root)
            .args(["rev-parse", "HEAD"]),
    )?;
    runner.run(
        Command::new("git")
            .current_dir(root)
            .args(["status", "--porcelain=v1"]),
    )?;
    digest(root.join("Cargo.lock").as_path(), runner)?;
    let checks_dir = runner.dir.join("checks");
    process::directory(&checks_dir)?;
    match options.action {
        Action::Check => {
            runner.run(&mut context.cargo(&["fmt", "--all", "--", "--check"]))?;
            for (package, target) in [
                ("xtask", context.host.as_str()),
                ("radishroot-kernel", TARGET),
            ] {
                for command in ["check", "clippy"] {
                    let mut cmd = context.cargo(&[
                        command,
                        "--locked",
                        "--offline",
                        "-p",
                        package,
                        "--target",
                        target,
                    ]);
                    cmd.arg("--target-dir").arg(&checks_dir);
                    if package == "xtask" {
                        cmd.arg("--all-targets");
                    }
                    if command == "clippy" {
                        cmd.args(["--", "-D", "warnings"]);
                    }
                    runner.run(&mut cmd)?;
                }
            }
        }
        Action::TestHost => {
            let output = runner.run(
                context
                    .cargo(&[
                        "test",
                        "--locked",
                        "--offline",
                        "-p",
                        "xtask",
                        "--target",
                        &context.host,
                    ])
                    .arg("--target-dir")
                    .arg(&checks_dir),
            )?;
            print!("{output}");
        }
        Action::Build { release, fresh } => {
            let target_dir = if fresh {
                let path = runner.dir.join("build");
                process::directory(&path)?;
                path
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
            digest(&artifact, runner)?;
            runner.record(&format!(
                "artifact={} entry={:#x} loads={} fresh={fresh} profile={}",
                artifact.display(),
                layout.entry,
                layout.loads,
                if release { "release" } else { "dev" }
            ))?;
            for (package, target) in [
                ("xtask", context.host.as_str()),
                ("radishroot-kernel", TARGET),
            ] {
                runner.run(&mut context.cargo(&[
                    "tree",
                    "--locked",
                    "--offline",
                    "-p",
                    package,
                    "--target",
                    target,
                    "-e",
                    "normal,build,dev,features",
                ]))?;
            }
            let metadata = runner.run(&mut context.cargo(&[
                "metadata",
                "--locked",
                "--offline",
                "--format-version",
                "1",
            ]))?;
            fs::write(runner.dir.join("metadata.json"), metadata)?;
            println!("verified static ELF: {}", artifact.display());
        }
        Action::Help => unreachable!(),
    }
    Ok(())
}
fn digest(path: &Path, runner: &mut Runner) -> Result<()> {
    let output = runner.run(Command::new("shasum").args(["-a", "256"]).arg(path))?;
    runner.record(output.trim())
}
