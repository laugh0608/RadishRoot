use super::*;
use crate::{
    boot_image::{self, Manifest},
    config::Context,
    vm::{self, BootSpec, Machine},
    vm_fat::{self, Expected, FatCheck},
    vm_storage,
};
fn fixture(pages: bool, complete: bool) -> Vec<Expected> {
    let mut files = vec![Expected {
        id: "seed",
        path: "seed.txt".into(),
        bytes: b"T10 workbench seed\n".to_vec(),
    }];
    if pages {
        for i in 0..200 {
            files.push(Expected {
                id: "page",
                path: format!("p{i:03}.txt"),
                bytes: b"T10 page item\n".to_vec(),
            });
        }
    }
    if complete {
        files.push(Expected {
            id: "note",
            path: "note.txt".into(),
            bytes: b"payload".to_vec(),
        });
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
) -> Result<()> {
    for mode in ["t10-write", "t10-file-sync", "t10-pages"] {
        let pages = mode == "t10-pages";
        let disk = vm_storage::clone_sparse(source, r)?;
        // 共用材料标签由工具入口按序号区分，路径和原始预期各自保留。
        let expected = fixture(pages, false);
        vm_fat::tools_with_files(
            c,
            r,
            m,
            &disk,
            FatCheck {
                mode: "prepare",
                files: expected,
                directories: vec![],
            },
        )?;
        let before = vm_fat::sha(&disk, r)?;
        let boot = boot_image::create(&c.root, r, m, kernel, mode)?;
        vm::boot(
            r,
            m,
            machine,
            BootSpec {
                boot_disk: &boot,
                data_disk: Some(&disk),
                readonly: pages,
                mode,
            },
        )?;
        let after = vm_fat::sha(&disk, r)?;
        if pages && before != after {
            return Err(bad("readonly pages disk changed"));
        }
        if mode == "t10-write" && before == after {
            return Err(bad("graphical write did not change disk"));
        }
        vm_fat::tools_with_files(
            c,
            r,
            m,
            &disk,
            FatCheck {
                mode: if mode == "t10-file-sync" {
                    "observe-file"
                } else {
                    "verify"
                },
                files: fixture(pages, mode == "t10-write"),
                directories: vec![],
            },
        )?;
        vm_fat::outside_unchanged(source, &disk, m)?;
        r.record(&format!("workbench_case={mode} typed_results=pass independent_DATA=checked outside_DATA=unchanged disk={}",disk.display()))?;
        println!("T10 {mode}: actual graphics input, typed results and independent disk passed");
    }
    r.record("T10 three graphics cases complete; no normal shutdown, full suite or M1 claim")
}
impl Session {
    pub(super) fn case_steps(&mut self) -> Result<()> {
        match self.mode.as_str() {
            "t10-write" => {
                self.command("write note.txt payload", 1)?;
                self.snapshot("workbench-created")?;
                self.command("write note.txt again", 2)?;
                self.click(590, 116)?;
                self.wait(|s| Ok(number(&fields(s)?, "details")? == 1))?;
                self.snapshot("workbench-conflict")?;
                self.tap("esc")?;
                self.command("cat note.txt", 3)?;
                self.snapshot("workbench-readback")?;
            }
            "t10-file-sync" => {
                self.command("write failed.txt payload", 1)?;
                self.click(590, 116)?;
                self.wait(|s| Ok(number(&fields(s)?, "details")? == 1))?;
                self.snapshot("workbench-partial-failure")?;
                self.tap("esc")?;
                self.command("ls", 2)?;
                self.click(40, 116)?;
                self.wait(|s| {
                    let f = fields(s)?;
                    Ok(number(&f, "pending")? == 0 && number(&f, "list_ok")? == 0)
                })?;
                self.snapshot("workbench-offline")?;
            }
            "t10-pages" => {
                for page in 1..=6 {
                    self.click(140, 116)?;
                    self.wait(|s| Ok(number(&fields(s)?, "pending")? == 0))?;
                    self.snapshot(&format!("workbench-page-{page}"))?;
                }
                let state = self.state()?;
                if number(&fields(&state)?, "entries")? != 9 {
                    return Err(bad("201 directory entries not reachable"));
                }
            }
            _ => return Err(bad("unknown workbench case")),
        }
        Ok(())
    }
}
pub fn validate(output: &str, mode: &str) -> Result<()> {
    let readonly = mode == "t10-pages";
    let device = output
        .lines()
        .find(|l| l.starts_with("RR:T04:DEVICE "))
        .ok_or_else(|| bad("missing actual storage device"))?;
    let f = fields(device)?;
    if number(&f, "sectors")? != 2097152
        || number(&f, "readonly")? != u64::from(readonly)
        || number(&f, "flush")? != 1
        || !output.contains("RR:T04:LIVE owner=file-service ")
    {
        return Err(bad("graphical storage device facts"));
    }
    let replies = output
        .lines()
        .filter_map(|l| l.strip_prefix("RR:T10:REPLY "))
        .map(|s| serde_json::from_str::<serde_json::Value>(s).map_err(|e| bad(e.to_string())))
        .collect::<Result<Vec<_>>>()?;
    if mode == "t10-pages" {
        if !replies.is_empty() {
            return Err(bad("unexpected command in list-only case"));
        }
        return Ok(());
    }
    let expected = if mode == "t10-write" { 3 } else { 2 };
    if replies.len() != expected {
        return Err(bad("missing graphical command reply"));
    }
    if mode == "t10-write" {
        if replies[0]["ok"] != true
            || replies[0]["output"]["kind"] != "created"
            || replies[1]["ok"] != false
            || replies[1]["error"]["detail"]["code"] != "AlreadyExists"
            || replies[2]["ok"] != true
            || replies[2]["output"]["hex"] != "7061796c6f6164"
        {
            return Err(bad("graphical create/conflict/raw bytes"));
        }
    } else if replies[0]["ok"] != false
        || replies[0]["error"]["detail"]["stage"] != "FileSync"
        || replies[0]["error"]["detail"]["partial_effect"]["persistence_confirmed"] != false
        || replies[0]["error"]["detail"]["partial_effect"]["may_have_changed"] != true
        || replies[1]["error"]["detail"]["code"] != "Offline"
    {
        return Err(bad(
            "graphical failure falsely succeeded / lost original stage",
        ));
    }
    let details = output
        .lines()
        .filter_map(|l| l.strip_prefix("RR:T10:DETAIL "))
        .map(|s| serde_json::from_str::<serde_json::Value>(s).map_err(|e| bad(e.to_string())))
        .collect::<Result<Vec<_>>>()?;
    let failed = if mode == "t10-write" {
        &replies[1]
    } else {
        &replies[0]
    };
    if details.len() != 1
        || details[0]["output"]["record"]["operation_id"] != failed["operation_id"]
        || details[0]["output"]["record"]["status"] != "Failed"
    {
        return Err(bad("graphical detail identity / failure"));
    }
    Ok(())
}
