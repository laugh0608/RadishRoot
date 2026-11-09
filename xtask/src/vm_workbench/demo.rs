//! T13 的正常演示编排，使用生产内核、实际图形输入和独立 FAT 读盘。
pub(super) mod manual;
use super::*;
use crate::{
    boot_image::Manifest,
    config::Context,
    vm::{self, Machine},
    vm_fat::{self, Expected, FatCheck},
    vm_storage,
};

const NAME: &str = "hello.txt";
const TARGET: &str = "/data/hello.txt";
const PAYLOAD: &str = "hello RadishRoot";

pub enum DemoBoot {
    Create,
    Reopen { old_operation: String },
}

fn fixture(complete: bool) -> Vec<Expected> {
    let mut files = cases::fixture(false, false);
    if complete {
        files.push(Expected {
            id: "hello",
            path: NAME.into(),
            bytes: PAYLOAD.as_bytes().to_vec(),
        });
    }
    files
}

pub fn execute(
    context: &Context,
    runner: &mut Runner,
    manifest: &Manifest,
    machine: &Machine,
    source: &Path,
    gui: bool,
) -> Result<()> {
    let disk = vm_storage::clone_sparse(source, runner)?;
    let check = |runner: &mut Runner, mode, complete| {
        vm_fat::tools_with_files(
            context,
            runner,
            manifest,
            &disk,
            FatCheck {
                mode,
                files: fixture(complete),
                directories: vec![],
            },
        )
    };
    check(runner, "prepare", false)?;
    check(runner, "verify", false)?;
    runner.record(&format!(
        "demo_stage=create_and_shutdown manual_cocoa={gui}"
    ))?;
    let first = vm::boot_demo(
        runner,
        manifest,
        machine,
        source,
        &disk,
        DemoBoot::Create,
        gui,
    )?;
    let replies = json_lines(&first, "RR:T10:REPLY ")?;
    let old_operation = replies
        .first()
        .and_then(|write| write["operation_id"].as_str())
        .ok_or_else(|| bad("missing first boot write identity"))?
        .to_owned();

    runner.record("demo_stage=independent_readback")?;
    check(runner, "verify", true)?;
    vm_fat::outside_unchanged(source, &disk, manifest)?;
    let saved = vm_fat::sha(&disk, runner)?;

    runner.record("demo_stage=reopen_same_DATA")?;
    vm::boot_demo(
        runner,
        manifest,
        machine,
        source,
        &disk,
        DemoBoot::Reopen { old_operation },
        gui,
    )?;
    if vm_fat::sha(&disk, runner)? != saved {
        return Err(bad("demo read-only reopen changed DATA disk"));
    }
    runner.record(&format!(
        "demo=verified data={} boots=2 independent_FAT=exact readonly_reopen=unchanged",
        disk.display()
    ))?;
    println!(
        "T13 demo verified: automatic file listing, conflict, normal shutdown, independent readback and same-DATA restart"
    );
    Ok(())
}

impl Session {
    pub(super) fn demo_poll(&mut self, serial: &str, frame: &str) -> Result<bool> {
        if (self.gui || self.power_done) && fields(frame)?.get("power") == Some(&"failed") {
            let cause = serial
                .lines()
                .rfind(|line| line.starts_with("RR:T12:BLOCKED "));
            let reply = serial
                .lines()
                .rfind(|line| line.starts_with("RR:T10:REPLY "));
            return Err(Error::new(
                "demo-shutdown",
                cause
                    .or(reply)
                    .unwrap_or("guest reported failed power state"),
            ));
        }
        if self.gui {
            return self.manual_demo_poll(serial, frame);
        }
        if self.power_done {
            return Ok(false);
        }

        let shutdown = match self.demo.as_ref().expect("demo session") {
            DemoBoot::Create => {
                self.command(&format!("write {TARGET} \"{PAYLOAD}\""), 1)?;
                self.wait_for_visible(NAME)?;
                self.snapshot("demo-automatic-file")?;
                self.source_details("demo-original-write-details")?;
                self.tap("esc")?;
                self.command(&format!("write {TARGET} again"), 2)?;
                self.click(590, 116)?;
                self.wait(|s| Ok(number(&fields(s)?, "details")? == 1))?;
                self.snapshot("demo-conflict-details")?;
                self.tap("esc")?;
                3
            }
            DemoBoot::Reopen { old_operation } => {
                let explain = format!("explain {old_operation}");
                self.command("ls /data", 1)?;
                self.snapshot("demo-reopened-list")?;
                self.command(&format!("cat {TARGET}"), 2)?;
                self.snapshot("demo-reopened-bytes")?;
                self.command(&explain, 3)?;
                self.snapshot("demo-foreign-operation")?;
                4
            }
        };
        self.command("shutdown", shutdown)?;
        self.snapshot("demo-shutdown-result")?;
        self.power_done = true;
        Ok(false)
    }
}

pub(super) fn validate(output: &str, demo: &DemoBoot, boot: [u8; 16]) -> Result<()> {
    cases::live_device(output, matches!(demo, DemoBoot::Reopen { .. }))?;
    let boot = format!("{:032x}", u128::from_be_bytes(boot));
    let identity = output
        .lines()
        .find(|line| line.starts_with("RR:T08:DEFAULT "))
        .ok_or_else(|| bad("demo guest boot identity missing"))?;
    if fields(identity)?.get("boot_id") != Some(&boot.as_str()) {
        return Err(bad("demo guest identity differs from allocated boot ID"));
    }
    let replies = json_lines(output, "RR:T10:REPLY ")?;
    let shutdown = match (demo, replies.as_slice()) {
        (DemoBoot::Create, [write, conflict, shutdown]) => {
            validate_create(output, write, conflict, &boot)?;
            shutdown
        }
        (DemoBoot::Reopen { old_operation }, [list, read, explain, shutdown]) => {
            validate_reopen(list, read, explain, old_operation, &boot)?;
            shutdown
        }
        _ => return Err(bad("unexpected demo replies")),
    };
    if shutdown["operation_id"]
        .as_str()
        .and_then(|id| id.split_once(':'))
        .is_none_or(|(id, _)| id != boot)
    {
        return Err(bad("demo shutdown operation has the wrong boot identity"));
    }
    power::validate_request(output, shutdown, "Shutdown")
}

fn validate_create(output: &str, write: &Value, conflict: &Value, boot: &str) -> Result<()> {
    let operation = write["operation_id"]
        .as_str()
        .ok_or_else(|| bad("demo write operation missing"))?;
    let sequence = operation
        .split_once(':')
        .filter(|(id, _)| *id == boot)
        .and_then(|(_, sequence)| sequence.parse::<u64>().ok())
        .filter(|sequence| *sequence > 0)
        .ok_or_else(|| bad("demo write boot identity or sequence"))?;
    let page = link::last_page(output)?;
    if write["ok"] != true
        || write["output"]["kind"] != "created"
        || write["output"]["reference"]["path"] != TARGET
        || write["output"]["reference"]["boot_id"] != boot
        || page.get("update") != Some(&"refreshed")
        || number(&page, "source")? != sequence
        || number(&page, "query")? == sequence
    {
        return Err(bad("demo creation or automatic list source differs"));
    }
    let details = json_lines(output, "RR:T10:DETAIL ")?;
    let [created, refused] = details.as_slice() else {
        return Err(bad("demo must capture creation and conflict details"));
    };
    let record = &created["output"]["record"];
    let mut events = created["output"]["events"]
        .as_array()
        .ok_or_else(|| bad("demo write events missing"))?
        .iter();
    if created["ok"] != true
        || record["operation_id"] != operation
        || record["request_kind"] != "CreateText"
        || record["status"] != "Succeeded"
        || record["partial_effect"]["written_bytes"] != PAYLOAD.len()
        || record["partial_effect"]["persistence_confirmed"] != true
        || ["Validate", "Write", "FileSync", "DeviceFlush", "Complete"]
            .iter()
            .any(|stage| !events.any(|event| event["stage"] == *stage))
    {
        return Err(bad(
            "demo write details missing original ID or writeback events",
        ));
    }
    let failure = &conflict["error"]["detail"];
    let refused_record = &refused["output"]["record"];
    if conflict["ok"] != false
        || failure["code"] != "AlreadyExists"
        || failure["object"] != TARGET
        || failure["partial_effect"]["may_have_changed"] != false
        || conflict["operation_id"]
            .as_str()
            .is_none_or(|id| id == operation)
        || refused["ok"] != true
        || refused_record["operation_id"] != conflict["operation_id"]
        || refused_record["status"] != "Failed"
        || refused_record["error"]["code"] != "AlreadyExists"
    {
        return Err(bad(
            "demo conflict lost refusal, original ID or side effects",
        ));
    }
    Ok(())
}

fn validate_reopen(
    list: &Value,
    read: &Value,
    explain: &Value,
    old: &str,
    boot: &str,
) -> Result<()> {
    let expected_hex = PAYLOAD
        .bytes()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let entries = list["output"]["entries"]
        .as_array()
        .ok_or_else(|| bad("demo reopened list missing"))?;
    if old.split_once(':').is_none_or(|(id, _)| id == boot)
        || list["ok"] != true
        || list["output"]["kind"] != "files"
        || list["output"]["more"] != false
        || entries.len() != 2
        || !entries.iter().any(|entry| {
            entry["name"] == NAME
                && entry["size_bytes"] == PAYLOAD.len()
                && entry["reference"]["path"] == TARGET
                && entry["reference"]["boot_id"] == boot
        })
        || read["ok"] != true
        || read["output"]["kind"] != "bytes"
        || read["output"]["length"] != PAYLOAD.len()
        || read["output"]["hex"] != expected_hex
        || read["output"]["offset"] != 0
        || read["output"]["eof"] != true
        || explain["ok"] != false
        || explain["error"]["kind"] != "ForeignBoot"
        || !explain["operation_id"].is_null()
    {
        return Err(bad(
            "demo same-disk readback or old operation rejection differs",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reopen_requires_original_bytes_and_foreign_boot_rejection() {
        let boot = "00000000000000000000000000000002";
        let previous = "00000000000000000000000000000001:3";
        let list = json!({"ok":true,"output":{"kind":"files","more":false,"entries":[
            {"name":"hello.txt","size_bytes":16,"reference":{"path":"/data/hello.txt","boot_id":boot}},
            {"name":"seed.txt"}
        ]}});
        let read = json!({"ok":true,"output":{"kind":"bytes","offset":0,"length":16,"eof":true,"hex":"68656c6c6f20526164697368526f6f74"}});
        let explain = json!({"ok":false,"operation_id":null,"error":{"kind":"ForeignBoot"}});
        validate_reopen(&list, &read, &explain, previous, boot).unwrap();
        let mut bad_read = read.clone();
        bad_read["output"]["hex"] = json!("68656c6c6f20526164697368526f6f75");
        assert!(validate_reopen(&list, &bad_read, &explain, previous, boot).is_err());
        let mut unavailable = explain.clone();
        unavailable["error"]["kind"] = json!("OperationUnavailable");
        assert!(validate_reopen(&list, &read, &unavailable, previous, boot).is_err());
        assert!(validate_reopen(&list, &read, &explain, &format!("{boot}:3"), boot).is_err());
    }
}
