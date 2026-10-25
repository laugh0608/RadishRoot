use crate::error::{Error, Result};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub fn directory(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        directory_existing(parent)?;
    }
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => directory_existing(path),
        Err(e) => Err(e.into()),
    }
}
fn directory_existing(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        let meta = fs::symlink_metadata(ancestor)?;
        if meta.file_type().is_symlink() || !meta.is_dir() {
            return Err(Error::new(
                "output-path",
                format!("not a real directory: {}", ancestor.display()),
            ));
        }
    }
    Ok(())
}
pub fn reject_output_symlinks(path: &Path) -> Result<()> {
    directory_existing(path)?;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(Error::new(
                "output-path",
                format!("symlink in output tree: {}", entry.path().display()),
            ));
        }
        if kind.is_dir() {
            reject_output_symlinks(&entry.path())?;
        } else if !kind.is_file() {
            return Err(Error::new("output-path", "non-regular output entry"));
        }
    }
    Ok(())
}
pub fn unique_directory(parent: &Path, prefix: &str) -> Result<PathBuf> {
    directory_existing(parent)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| Error::new("clock", e.to_string()))?
        .as_nanos();
    let path = parent.join(format!("{prefix}-{}-{stamp}", std::process::id()));
    fs::create_dir(&path)?; // collision is a failure; never reuse or delete another run
    Ok(path)
}
fn create(path: &Path) -> Result<File> {
    Ok(OpenOptions::new().write(true).create_new(true).open(path)?)
}
fn read_output(path: &Path) -> Result<String> {
    let file = File::open(path)?;
    if file.metadata()?.len() > 2 * 1024 * 1024 {
        return Err(Error::new(
            "output-limit",
            "child output exceeds 2 MiB; see run log",
        ));
    }
    let mut text = String::new();
    file.take(2 * 1024 * 1024 + 1).read_to_string(&mut text)?;
    Ok(text)
}

pub struct Runner {
    pub dir: PathBuf,
    pub timeout: Duration,
    sequence: usize,
    report: File,
}
impl Runner {
    pub fn new(dir: PathBuf, timeout: Duration) -> Result<Self> {
        let mut report = create(&dir.join("steps.txt"))?;
        writeln!(report, "timeout_seconds={}", timeout.as_secs_f64())?;
        Ok(Self {
            dir,
            timeout,
            sequence: 0,
            report,
        })
    }
    pub fn record(&mut self, line: &str) -> Result<()> {
        writeln!(self.report, "{line}")?;
        self.report.flush()?;
        Ok(())
    }
    pub fn run(&mut self, command: &mut Command) -> Result<String> {
        self.run_inner(command, None)
    }
    pub fn boot(&mut self, command: &mut Command, serial: &Path) -> Result<String> {
        self.run_inner(command, Some(serial))
    }
    fn run_inner(&mut self, command: &mut Command, serial: Option<&Path>) -> Result<String> {
        self.sequence += 1;
        let prefix = format!("{:03}", self.sequence);
        let stdout = self.dir.join(format!("{prefix}.stdout"));
        let stderr = self.dir.join(format!("{prefix}.stderr"));
        self.record(&format!(
            "{prefix} program={:?} args={:?}",
            command.get_program(),
            command.get_args().collect::<Vec<_>>()
        ))?;
        command
            .env("RUSTUP_AUTO_INSTALL", "0")
            .stdin(Stdio::null())
            .stdout(create(&stdout)?)
            .stderr(create(&stderr)?)
            .process_group(0);
        let started = Instant::now();
        let mut child = command
            .spawn()
            .map_err(|e| Error::new("spawn", e.to_string()))?;
        let pid = child.id();
        // The child creates its own process group before exec. Only this group is signalled.
        let mut timed_out = false;
        let mut observed = false;
        let mut terminal_since = None;
        let mut prior_length = 0;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if started.elapsed() < self.timeout => {
                    if let Some(path) = serial {
                        let output = match read_output(path) {
                            Ok(output) => output,
                            Err(error) => {
                                stop_and_wait(&mut child, pid)?;
                                return Err(error);
                            }
                        };
                        let terminal = output.lines().any(|line| {
                            line.starts_with("RR:T03:READY ")
                                || line.starts_with("RR:T03:FAIL ")
                                || line.starts_with("RR:T02:FAIL ")
                        });
                        if terminal && (terminal_since.is_none() || output.len() != prior_length) {
                            terminal_since = Some(Instant::now());
                        }
                        prior_length = output.len();
                        if terminal_since.is_some_and(|t| t.elapsed() >= Duration::from_millis(250))
                        {
                            observed = true;
                            break stop_and_wait(&mut child, pid)?;
                        }
                    }
                    thread::sleep(Duration::from_millis(10))
                }
                Ok(None) => {
                    timed_out = true;
                    break stop_and_wait(&mut child, pid)?;
                }
                Err(e) => {
                    stop_and_wait(&mut child, pid)?;
                    return Err(Error::new("wait", e.to_string()));
                }
            }
        };
        // Also stop any descendants left behind by an exited parent.
        kill_group(pid)?;
        self.record(&format!(
            "{prefix} pid={pid} code={:?} signal={:?} timeout={timed_out} host_stopped_after_terminal={observed} elapsed_ms={}",
            status.code(),
            status.signal(),
            started.elapsed().as_millis()
        ))?;
        if timed_out {
            return Err(Error {
                kind: "timeout",
                detail: format!("child group {pid} stopped; logs: {}", self.dir.display()),
                exit: 124,
            });
        }
        if let Some(path) = serial {
            if !observed {
                return Err(Error::new(
                    "vm-exited",
                    "guest exited before terminal observation; not a successful shutdown",
                ));
            }
            return read_output(path);
        }
        if !status.success() {
            return Err(Error::new(
                "child-failed",
                format!(
                    "code={:?} signal={:?}; logs: {}\n{}",
                    status.code(),
                    status.signal(),
                    self.dir.display(),
                    read_output(&stderr)?
                ),
            ));
        }
        read_output(&stdout)
    }
}
fn kill_group(pid: u32) -> Result<()> {
    let output = Command::new("/bin/kill")
        .args(["-KILL", "--", &format!("-{pid}")])
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()?;
    // Do not confuse permission/argument failures with a group that already exited.
    let message = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() && !message.contains("No such process") {
        return Err(Error::new(
            "reap",
            format!("cannot signal task process group {pid}: {message}"),
        ));
    }
    Ok(())
}
fn stop_and_wait(child: &mut std::process::Child, pid: u32) -> Result<std::process::ExitStatus> {
    kill_group(pid)?;
    Ok(child.wait()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn runner() -> Runner {
        let parent = std::env::temp_dir().canonicalize().unwrap();
        Runner::new(
            unique_directory(&parent, "radishroot-process-test").unwrap(),
            Duration::from_millis(150),
        )
        .unwrap()
    }
    #[test]
    fn child_failure_and_spawn_failure_are_errors() {
        let mut r = runner();
        let e = r
            .run(Command::new("/bin/sh").args(["-c", "exit 7"]))
            .unwrap_err();
        assert_eq!(e.kind, "child-failed");
        assert!(e.detail.contains("Some(7)"));
        assert_eq!(
            r.run(&mut Command::new(r.dir.join("absent")))
                .unwrap_err()
                .kind,
            "spawn"
        );
        fs::remove_dir_all(&r.dir).unwrap();
    }
    #[test]
    fn signal_and_output_creation_failure_are_not_success() {
        let mut r = runner();
        let error = r
            .run(Command::new("/bin/sh").args(["-c", "kill -TERM $$"]))
            .unwrap_err();
        assert_eq!(error.kind, "child-failed");
        assert!(error.detail.contains("signal=Some(15)"));
        let conflict = r.dir.join("002.stdout");
        fs::write(&conflict, "existing evidence").unwrap();
        assert!(r.run(&mut Command::new("/usr/bin/true")).is_err());
        assert_eq!(fs::read_to_string(&conflict).unwrap(), "existing evidence");
        fs::remove_dir_all(&r.dir).unwrap();
    }
    #[test]
    fn timeout_stops_descendant_before_delayed_write() {
        let mut r = runner();
        let marker = r.dir.join("unexpected");
        let e = r
            .run(
                Command::new("/bin/sh")
                    .args(["-c", "(sleep 0.5; touch \"$1\") & wait", "test"])
                    .arg(&marker),
            )
            .unwrap_err();
        assert_eq!(e.exit, 124);
        thread::sleep(Duration::from_millis(600));
        assert!(!marker.exists(), "descendant survived group timeout");
        fs::remove_dir_all(&r.dir).unwrap();
    }
    #[test]
    fn boot_monitor_requires_terminal_then_stops_only_owned_child() {
        let mut r = runner();
        r.timeout = Duration::from_secs(2);
        let serial = r.dir.join("serial.log");
        fs::write(&serial, "").unwrap();
        let output = r
            .boot(
                Command::new("/bin/sh")
                    .args([
                        "-c",
                        "echo 'RR:T03:READY storage_io=none' > \"$1\"; sleep 5",
                        "test",
                    ])
                    .arg(&serial),
                &serial,
            )
            .unwrap();
        assert!(output.contains("READY"));
        fs::write(&serial, "").unwrap();
        assert_eq!(
            r.boot(Command::new("/bin/sh").args(["-c", "exit 0"]), &serial)
                .unwrap_err()
                .kind,
            "vm-exited"
        );
        r.timeout = Duration::from_millis(100);
        assert_eq!(
            r.boot(Command::new("/bin/sh").args(["-c", "sleep 5"]), &serial)
                .unwrap_err()
                .exit,
            124
        );
        fs::remove_dir_all(&r.dir).unwrap();
    }
    #[test]
    fn output_symlink_is_rejected() {
        let r = runner();
        let link = r.dir.join("link");
        std::os::unix::fs::symlink(std::env::temp_dir(), &link).unwrap();
        assert_eq!(directory(&link).unwrap_err().kind, "output-path");
        assert!(unique_directory(&link, "forbidden").is_err());
        assert!(reject_output_symlinks(&r.dir).is_err());
        fs::remove_dir_all(&r.dir).unwrap();
    }
}
