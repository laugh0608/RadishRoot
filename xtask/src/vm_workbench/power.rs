//! T12：只发送真实图形命令；正常退出必须由 QMP 与进程退出共同证明。
use super::*;
use crate::{
    boot_image::{self, Manifest},
    config::Context,
    vm::{self, BootSpec, Machine},
    vm_fat::{self, Expected, FatCheck},
    vm_storage,
};
pub const CASES: [&str; 7] = [
    "t12-sync",
    "t12-shutdown",
    "t12-reboot",
    "t12-volume-fail",
    "t12-flush-fail",
    "t12-platform-fail",
    "t12-readonly",
];
pub(super) fn failure(mode: &str) -> bool {
    matches!(
        mode,
        "t12-volume-fail" | "t12-flush-fail" | "t12-platform-fail"
    )
}
fn fixture(mode: &str, complete: bool) -> Vec<Expected> {
    let mut files = cases::fixture(false, false);
    if complete && mode != "t12-readonly" {
        files.push(Expected {
            id: "power",
            path: "power.txt".into(),
            bytes: b"payload".to_vec(),
        });
        if mode == "t12-sync" {
            files.push(Expected {
                id: "after",
                path: "after.txt".into(),
                bytes: b"yes".to_vec(),
            });
        }
    }
    files
}
pub fn execute(
    c: &Context,
    r: &mut Runner,
    m: &Manifest,
    machine: &Machine,
    kernel: &Path,
    source: &Path,
    selected: &[&str],
) -> Result<()> {
    for &mode in selected {
        let disk = vm_storage::clone_sparse(source, r)?;
        vm_fat::tools_with_files(
            c,
            r,
            m,
            &disk,
            FatCheck {
                mode: "prepare",
                files: fixture(mode, false),
                directories: vec![],
            },
        )?;
        let boot = boot_image::create(&c.root, r, m, kernel, mode)?;
        vm::boot(
            r,
            m,
            machine,
            BootSpec {
                boot_disk: &boot,
                data_disk: Some(&disk),
                readonly: mode == "t12-readonly",
                mode,
            },
        )?;
        if mode == "t12-reboot" {
            let reopen = boot_image::create(&c.root, r, m, kernel, "t12-reopen")?;
            vm::boot(
                r,
                m,
                machine,
                BootSpec {
                    boot_disk: &reopen,
                    data_disk: Some(&disk),
                    readonly: false,
                    mode: "t12-reopen",
                },
            )?;
            r.record("power_reboot_chain=guest-reset_then_same_DATA_new_process_and_boot_id")?;
        }
        vm_fat::tools_with_files(
            c,
            r,
            m,
            &disk,
            FatCheck {
                mode: "verify",
                files: fixture(mode, true),
                directories: vec![],
            },
        )?;
        vm_fat::outside_unchanged(source, &disk, m)?;
        r.record(&format!("power_case={mode} typed_results=pass independent_DATA=exact outside_DATA=unchanged natural_exit={}",!failure(mode)))?;
    }
    r.record(if selected==CASES {"T12 seven power cases, eight VM processes complete; controlled reboot uses two processes and same DATA; no complete AT14 or M1 claim"} else {"T12 selected power cases complete; not the full power group, AT14 or M1"})
}
impl Session {
    pub(super) fn power_poll(&mut self) -> Result<bool> {
        if self.power_done {
            return Ok(false);
        }
        let mode = self.mode.clone();
        let mut count = 0;
        if mode == "t12-reopen" {
            count += 1;
            self.command("cat power.txt", count)?;
            self.snapshot("reboot-original-bytes")?;
        } else if mode != "t12-readonly" {
            count += 1;
            self.command("write power.txt payload", count)?;
        }
        if matches!(mode.as_str(), "t12-sync" | "t12-readonly") {
            count += 1;
            self.command("sync", count)?;
            self.snapshot("explicit-sync")?;
            if mode == "t12-sync" {
                count += 1;
                self.command("write after.txt yes", count)?;
            }
        }
        count += 1;
        self.command(
            if mode == "t12-reboot" {
                "reboot"
            } else {
                "shutdown"
            },
            count,
        )?;
        self.snapshot("power-command-result")?;
        if failure(&mode) {
            self.wait(|s| Ok(fields(s)?.get("power") == Some(&"failed")))?;
            self.snapshot("power-failed-live")?;
            self.click(590, 116)?;
            self.wait(|s| Ok(number(&fields(s)?, "details")? == 1))?;
            self.snapshot("power-failed-original-details")?;
            self.tap("esc")?;
            count += 1;
            self.command("write late.txt x", count)?;
            count += 1;
            self.command("help", count)?;
            self.snapshot("power-failed-still-diagnostic")?;
            return Ok(true);
        }
        self.power_done = true;
        Ok(false)
    }
}
fn json_replies(output: &str, prefix: &str) -> Result<Vec<serde_json::Value>> {
    output
        .lines()
        .filter_map(|s| s.strip_prefix(prefix))
        .map(|s| serde_json::from_str(s).map_err(|e| bad(e.to_string())))
        .collect()
}
pub(super) fn validate(output: &str, mode: &str) -> Result<()> {
    if !CASES.contains(&mode) && mode != "t12-reopen" {
        return Err(bad("unknown power mode"));
    }
    cases::live_device(output, mode == "t12-readonly")?;
    let replies = json_replies(output, "RR:T10:REPLY ")?;
    let power = replies
        .iter()
        .find(|r| {
            r["output"]["kind"] == "power-prepared"
                || r["error"]["detail"]["stage"] == "VolumeSync"
                || r["error"]["detail"]["stage"] == "DeviceFlush"
        })
        .ok_or_else(|| bad("missing power operation reply"))?;
    if mode != "t12-readonly"
        && mode != "t12-reopen"
        && (replies[0]["ok"] != true
            || replies[0]["output"]["reference"]["path"] != "/data/power.txt")
    {
        return Err(bad("power fixture was not created by guest"));
    }
    if mode == "t12-reopen"
        && (replies[0]["output"]["hex"] != "7061796c6f6164" || replies[0]["output"]["length"] != 7)
    {
        return Err(bad("reboot did not read original bytes"));
    }
    if matches!(mode, "t12-sync" | "t12-readonly")
        && !replies.iter().any(|r| {
            r["ok"] == true
                && r["output"]["kind"] == "synced"
                && r["output"]["persistence_confirmed"] == true
        })
    {
        return Err(bad("explicit sync missing"));
    }
    if failure(mode) {
        if output.contains("RR:T12:REQUEST ") || output.contains("RR:T12:STOP ") {
            return Err(bad("failed power request reached hardware"));
        }
        if !replies
            .iter()
            .any(|r| r["ok"] == false && r["error"]["detail"]["code"] == "ShuttingDown")
            || replies.last().unwrap()["output"]["kind"] != "help"
        {
            return Err(bad(
                "failed power did not keep writes blocked and diagnostics working",
            ));
        }
        let details = json_replies(output, "RR:T10:DETAIL ")?;
        let expected = match mode {
            "t12-volume-fail" => "VolumeSync",
            "t12-flush-fail" => "DeviceFlush",
            _ => "Platform",
        };
        if details.len() != 1
            || details[0]["output"]["record"]["operation_id"] != power["operation_id"]
            || details[0]["output"]["record"]["status"] != "Failed"
            || details[0]["output"]["record"]["error"]["stage"] != expected
        {
            return Err(bad("power failure lost original operation/stage"));
        }
    } else {
        if power["ok"] != true || power["output"]["power_completed"] != false {
            return Err(bad("prepared power claimed completion"));
        }
        let prepared = output
            .find("RR:T12:PREPARED ")
            .ok_or_else(|| bad("no synchronized prepare"))?;
        let stop = output
            .find("RR:T12:STOP reset=confirmed bus_master=0 allocations=0 shared=0")
            .ok_or_else(|| bad("device not safely stopped"))?;
        let request = output
            .find("RR:T12:REQUEST ")
            .ok_or_else(|| bad("no platform request"))?;
        if !(prepared < stop && stop < request)
            || output.contains("RR:T12:BLOCKED ")
            || output.matches("RR:T12:REQUEST ").count() != 1
        {
            return Err(bad("power ordering/failure"));
        }
        let request_line = output[request..].lines().next().unwrap();
        let fields = fields(request_line)?;
        let sequence = power["operation_id"]
            .as_str()
            .unwrap()
            .rsplit(':')
            .next()
            .unwrap()
            .parse::<u64>()
            .map_err(|_| bad("power ID"))?;
        if number(&fields, "operation")? != sequence
            || fields.get("action")
                != Some(&if mode == "t12-reboot" {
                    "Reboot"
                } else {
                    "Shutdown"
                })
        {
            return Err(bad("power platform ID/action"));
        }
    }
    Ok(())
}
