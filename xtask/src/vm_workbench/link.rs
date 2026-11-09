//! T11 自动联动：独立预期核对图形与串口写入、原历史和更新来源，不发送手动刷新。
use super::*;
use crate::{
    boot_image::{self, Manifest},
    config::Context,
    vm::{self, BootSpec, Machine},
    vm_fat::{self, Expected, FatCheck},
    vm_storage,
};
const CASES: [&str; 4] = ["t11-link", "t11-pages", "t11-file-sync", "t11-external"];
fn target(mode: &str) -> &'static str {
    match mode {
        "t11-link" => "linked.txt",
        "t11-pages" => "later.txt",
        "t11-file-sync" => "failed.txt",
        "t11-external" => "external.txt",
        _ => unreachable!("known T11 case"),
    }
}
fn fixture(mode: &str, complete: bool) -> Vec<Expected> {
    let mut files = cases::fixture(mode == "t11-pages", false);
    if complete {
        files.push(Expected {
            id: "linked",
            path: target(mode).into(),
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
    for mode in CASES {
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
                readonly: false,
                mode,
            },
        )?;
        vm_fat::tools_with_files(
            c,
            r,
            m,
            &disk,
            FatCheck {
                mode: if mode == "t11-file-sync" {
                    "observe-file"
                } else {
                    "verify"
                },
                files: fixture(mode, mode != "t11-file-sync"),
                directories: vec![],
            },
        )?;
        vm_fat::outside_unchanged(source, &disk, m)?;
        r.record(&format!("link_case={mode} automatic_list=pass original_operation=pass independent_DATA=checked outside_DATA=unchanged disk={}",disk.display()))?;
        println!("T11 {mode}: automatic list, original operation and independent disk passed");
    }
    r.record("T11 four automatic-link cases complete; no manual refresh, full suite or normal power claim")
}
impl Session {
    fn visible_target(&self, mode: &str) -> Result<()> {
        self.wait(|s| {
            let f = fields(s)?;
            Ok(number(&f, "pending")? == 0 && f.get("update") == Some(&"refreshed"))
        })?;
        let text = read_serial(&self.serial)?;
        visible(&text, target(mode))
    }
    fn source_details(&mut self, name: &str) -> Result<()> {
        self.click(40, 170)?;
        self.wait(|s| Ok(number(&fields(s)?, "details")? == 1))?;
        self.snapshot(name)
    }
    fn external_write(&self) -> Result<()> {
        let socket = self
            .console_socket
            .as_ref()
            .ok_or_else(|| bad("external console socket missing"))?;
        let mut console = crate::vm_shell::Session::for_input(
            socket,
            &self.serial,
            &self.directory,
            b"write external.txt payload\r".to_vec(),
        )?;
        while !console.poll()? {
            if Instant::now() >= self.deadline {
                return Err(bad("external input case deadline"));
            }
            thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }
    pub(super) fn link_steps(&mut self) -> Result<()> {
        match self.mode.as_str() {
            "t11-link" => {
                self.command("ls", 1)?;
                self.command("write linked.txt payload", 2)?;
                self.visible_target("t11-link")?;
                self.snapshot("automatic-created")?;
                self.source_details("original-write-details")?;
                self.tap("esc")?;
                self.click(440, 116)?;
                self.snapshot("old-list-snapshot")?;
                self.tap("esc")?;
                let before = read_serial(&self.serial)?
                    .lines()
                    .filter(|l| l.starts_with("RR:T11:PAGE "))
                    .count();
                self.command("write linked.txt again", 3)?;
                let after = read_serial(&self.serial)?
                    .lines()
                    .filter(|l| l.starts_with("RR:T11:PAGE "))
                    .count();
                if before != after {
                    return Err(bad("pure conflict triggered list reload"));
                }
                self.click(590, 116)?;
                self.wait(|s| Ok(number(&fields(s)?, "details")? == 1))?;
                self.snapshot("conflict-original-details")?;
            }
            "t11-pages" => {
                self.command("ls", 1)?;
                self.command("write later.txt payload", 2)?;
                self.visible_target("t11-pages")?;
                self.snapshot("automatic-last-page")?;
                self.source_details("paged-write-details")?;
                self.tap("esc")?;
                self.click(440, 116)?;
                self.snapshot("paged-old-list-snapshot")?;
            }
            "t11-file-sync" => {
                self.command("write failed.txt payload", 1)?;
                self.wait(|s| {
                    let f = fields(s)?;
                    Ok(f.get("update") == Some(&"failed")
                        && number(&f, "list_ok")? == 0
                        && number(&f, "pending")? == 0)
                })?;
                self.snapshot("automatic-refresh-failed")?;
                self.source_details("failed-write-source")?;
                self.tap("esc")?;
                let before = read_serial(&self.serial)?
                    .lines()
                    .filter(|l| l.starts_with("RR:T11:PAGE "))
                    .count();
                self.command("help", 2)?;
                if read_serial(&self.serial)?
                    .lines()
                    .filter(|l| l.starts_with("RR:T11:PAGE "))
                    .count()
                    != before
                {
                    return Err(bad("offline refresh retried without change"));
                }
                self.snapshot("offline-no-retry")?;
            }
            "t11-external" => {
                self.command("ls", 1)?;
                self.external_write()?;
                self.visible_target("t11-external")?;
                self.snapshot("external-write-visible")?;
                self.source_details("external-original-operation")?;
            }
            _ => return Err(bad("unknown T11 case")),
        }
        Ok(())
    }
}
fn last_page(output: &str) -> Result<std::collections::BTreeMap<&str, &str>> {
    fields(
        output
            .lines()
            .rfind(|l| l.starts_with("RR:T11:PAGE "))
            .ok_or_else(|| bad("missing automatic page report"))?,
    )
}
fn visible(output: &str, name: &str) -> Result<()> {
    let page = last_page(output)?;
    let query = number(&page, "query")?;
    let expected = name.bytes().map(|b| format!("{b:02x}")).collect::<String>();
    if page.get("update") != Some(&"refreshed")
        || number(&page, "ok")? != 1
        || number(&page, "generation")? != 1
        || number(&page, "source")? == 0
        || number(&page, "source")? == query
    {
        return Err(bad("automatic page status/source"));
    }
    let mut names = vec![];
    for line in output.lines().filter(|l| l.starts_with("RR:T11:ITEM ")) {
        let f = fields(line)?;
        if number(&f, "query")? == query {
            names.push(
                f.get("name")
                    .copied()
                    .ok_or_else(|| bad("missing item name"))?,
            );
        }
    }
    let at = names
        .iter()
        .position(|n| *n == expected)
        .ok_or_else(|| bad("new file not in automatic list"))?;
    let frame = fields(
        output
            .lines()
            .rfind(|l| l.starts_with("RR:T10:FRAME "))
            .ok_or_else(|| bad("missing selected-file frame"))?,
    )?;
    let row = number(&frame, "file_row")? as usize;
    let scroll = number(&frame, "file_scroll")? as usize;
    if row != at || scroll > at || at - scroll >= 22 {
        return Err(bad("new file not selected / visible"));
    }
    Ok(())
}
fn json_lines(output: &str, prefix: &str) -> Result<Vec<serde_json::Value>> {
    output
        .lines()
        .filter_map(|l| l.strip_prefix(prefix))
        .map(|s| serde_json::from_str(s).map_err(|e| bad(e.to_string())))
        .collect()
}
pub fn validate(output: &str, mode: &str) -> Result<()> {
    if !CASES.contains(&mode) {
        return Err(bad("unknown link mode"));
    }
    cases::live_device(output, false)?;
    let graph = json_lines(output, "RR:T10:REPLY ")?;
    let details = json_lines(output, "RR:T10:DETAIL ")?;
    let external = json_lines(output, "RR:CONSOLE:REPLY ")?;
    let (write, failed) = match mode {
        "t11-link" => {
            if graph.len() != 3
                || graph[0]["output"]["entries"].as_array().map(Vec::len) != Some(1)
                || graph[2]["ok"] != false
                || graph[2]["error"]["detail"]["code"] != "AlreadyExists"
            {
                return Err(bad("link history/conflict facts"));
            }
            (&graph[1], false)
        }
        "t11-pages" => {
            if graph.len() != 2
                || graph[0]["output"]["entries"].as_array().map(Vec::len) != Some(32)
            {
                return Err(bad("paged original history facts"));
            }
            (&graph[1], false)
        }
        "t11-file-sync" => {
            if graph.len() != 2
                || graph[0]["ok"] != false
                || graph[0]["error"]["detail"]["stage"] != "FileSync"
                || graph[0]["error"]["detail"]["partial_effect"]["persistence_confirmed"] != false
                || graph[1]["output"]["kind"] != "help"
            {
                return Err(bad("failed write/help facts"));
            }
            (&graph[0], true)
        }
        "t11-external" => {
            if graph.len() != 1
                || external.len() != 1
                || graph[0]["output"]["entries"].as_array().map(Vec::len) != Some(1)
            {
                return Err(bad("external write not separate from graphical history"));
            }
            (&external[0], false)
        }
        _ => unreachable!(),
    };
    let page = last_page(output)?;
    let operation = write["operation_id"]
        .as_str()
        .ok_or_else(|| bad("write operation ID missing"))?;
    let sequence = operation
        .rsplit(':')
        .next()
        .unwrap()
        .parse::<u64>()
        .map_err(|_| bad("write operation sequence"))?;
    if number(&page, "source")? != sequence || number(&page, "query")? == sequence {
        return Err(bad("list source is not original write operation"));
    }
    if failed {
        if page.get("update") != Some(&"failed") || number(&page, "ok")? != 0 {
            return Err(bad("failed refresh reported successful"));
        }
    } else {
        if write["ok"] != true
            || write["output"]["kind"] != "created"
            || write["output"]["reference"]["path"] != format!("/data/{}", target(mode))
        {
            return Err(bad("write target/result"));
        }
        visible(output, target(mode))?;
    }
    let expected_details = if mode == "t11-link" { 2 } else { 1 };
    if details.len() != expected_details
        || details[0]["ok"] != true
        || details[0]["output"]["record"]["operation_id"] != operation
        || details[0]["output"]["record"]["request_kind"] != "CreateText"
        || details[0]["output"]["record"]["status"] != if failed { "Failed" } else { "Succeeded" }
    {
        return Err(bad("source details changed operation identity or status"));
    }
    if mode == "t11-link"
        && (details[1]["output"]["record"]["operation_id"] != graph[2]["operation_id"]
            || details[1]["output"]["record"]["status"] != "Failed")
    {
        return Err(bad("conflict details lost original identity"));
    }
    Ok(())
}
