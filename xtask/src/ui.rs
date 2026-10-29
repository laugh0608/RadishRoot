use crate::{
    config::Context,
    error::{Error, Result},
    process::{self, Runner},
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

fn sdl(runner: &mut Runner) -> Result<PathBuf> {
    // 当前固定验收宿主，仅复用批准的原生库；不调用包管理器、不安装或改全局配置。
    let prefix = Path::new("/opt/homebrew/opt/sdl2-compat").canonicalize()?;
    let sdl3 = Path::new("/opt/homebrew/opt/sdl3").canonicalize()?;
    if prefix.file_name().and_then(|s| s.to_str()) != Some("2.32.72")
        || sdl3.file_name().and_then(|s| s.to_str()) != Some("3.4.16")
    {
        return Err(Error::new(
            "sdl-version",
            "expected approved sdl2-compat 2.32.72 / SDL3 3.4.16",
        ));
    }
    for root in [&prefix, &sdl3] {
        let receipt = fs::read_to_string(root.join("INSTALL_RECEIPT.json"))?;
        let receipt: serde_json::Value =
            serde_json::from_str(&receipt).map_err(|e| Error::new("sdl-receipt", e.to_string()))?;
        if receipt.get("poured_from_bottle").and_then(|v| v.as_bool()) != Some(true) {
            return Err(Error::new(
                "sdl-source",
                "expected existing Homebrew bottle",
            ));
        }
        runner.record(&format!("native_library_prefix={}", root.display()))?;
    }
    runner.run(Command::new(prefix.join("bin/sdl2-config")).arg("--version"))?;
    for lib in [
        prefix.join("lib/libSDL2.dylib"),
        sdl3.join("lib/libSDL3.dylib"),
    ] {
        crate::digest(&lib, runner)?;
    }
    Ok(prefix.join("lib"))
}
pub fn build(context: &Context, runner: &mut Runner, gui: bool) -> Result<PathBuf> {
    let target = context.root.join("target/host");
    process::directory(&target)?;
    let mut command = context.cargo(&[
        "build",
        "--locked",
        "--offline",
        "-p",
        "radishroot-host-runner",
        "--target",
        &context.host,
    ]);
    command.arg("--target-dir").arg(&target);
    if gui {
        let library = sdl(runner)?;
        command.args(["--features", "gui"]).env(
            "CARGO_ENCODED_RUSTFLAGS",
            format!(
                "-L\u{1f}native={}\u{1f}-C\u{1f}link-arg=-Wl,-rpath,{}",
                library.display(),
                library.display()
            ),
        );
    }
    let previous = runner.timeout;
    runner.timeout = Duration::from_secs(600);
    let result = runner.run(&mut command);
    runner.timeout = previous;
    result?;
    Ok(target
        .join(&context.host)
        .join("debug/radishroot-host-runner"))
}
pub fn execute(context: &Context, runner: &mut Runner, gui: bool) -> Result<()> {
    let binary = build(context, runner, gui)?;
    let output = process::unique_directory(&runner.dir, "host-view")?;
    // macOS 原生窗口需要实际 bundle 身份，便于窗口系统识别与交互验收；仅生成本次私有产物。
    let binary = if gui {
        let app = output.join("RadishRoot-T07.app");
        process::directory(&app)?;
        let contents = app.join("Contents");
        process::directory(&contents)?;
        let executable = contents.join("MacOS");
        process::directory(&executable)?;
        fs::write(
            contents.join("Info.plist"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>CFBundleIdentifier</key><string>org.radishroot.t07-host</string><key>CFBundleName</key><string>RadishRoot T07</string><key>CFBundleExecutable</key><string>radishroot-host-runner</string><key>CFBundlePackageType</key><string>APPL</string><key>NSHighResolutionCapable</key><true/></dict></plist>\n",
        )?;
        let target = executable.join("radishroot-host-runner");
        fs::copy(binary, &target)?;
        target
    } else {
        binary
    };
    let mut command = Command::new(binary);
    command.arg(&output);
    if gui {
        command.arg("--gui");
    }
    let previous = runner.timeout;
    if gui {
        runner.timeout = Duration::from_secs(600);
    }
    let result = runner.run(&mut command);
    runner.timeout = previous;
    print!("{}", result?);
    runner.record(&format!(
        "host_view={} gui={gui}; host rendering is not QEMU evidence",
        output.display()
    ))
}
