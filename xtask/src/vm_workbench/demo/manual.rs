//! Cocoa 验收由操作者交互，宿主只预填次窗 explain 的长前缀。
use super::*;
use std::collections::BTreeMap;

const REOPENED_BYTES: &str = "demo-reopened-bytes";
const PREFILLED: &str = "demo-explain-prefilled";
const FOREIGN_OPERATION: &str = "demo-foreign-operation";

struct Checkpoint {
    name: &'static str,
    results: u64,
    details: u64,
    focus: &'static str,
    next: &'static str,
}

const CREATE: &[Checkpoint] = &[
    Checkpoint {
        name: "demo-automatic-file",
        results: 1,
        details: 0,
        focus: "Input",
        next: "Click the selected hello.txt row to open the original operation details. Hold the pointer still until recorded.",
    },
    Checkpoint {
        name: "demo-original-write-details",
        results: 1,
        details: 1,
        focus: "CloseDetails",
        next: "Compare the operation ID and stages. Press Tab once to highlight Up in the details pane, then wait.",
    },
    Checkpoint {
        name: "demo-details-tab-focus",
        results: 1,
        details: 1,
        focus: "DetailsUp",
        next: "Press Shift+Tab once to return to Close, release Shift, then press Enter and wait.",
    },
    Checkpoint {
        name: "demo-details-keyboard-close",
        results: 1,
        details: 0,
        focus: "Input",
        next: "Enter: write /data/hello.txt again. After AlreadyExists, click Details in the terminal and wait.",
    },
    Checkpoint {
        name: "demo-conflict-details",
        results: 2,
        details: 1,
        focus: "CloseDetails",
        next: "Check the refusal and its operation ID. Press Esc, then enter: shutdown. Wait for disk verification and the second window.",
    },
];

const REOPEN: &[Checkpoint] = &[
    Checkpoint {
        name: "demo-reopened-list",
        results: 1,
        details: 0,
        focus: "Input",
        next: "Confirm hello.txt is listed, then enter: cat /data/hello.txt. Wait.",
    },
    Checkpoint {
        name: REOPENED_BYTES,
        results: 2,
        details: 0,
        focus: "Input",
        next: "Check the file contents and wait for the host to prefill the explain draft.",
    },
    Checkpoint {
        name: PREFILLED,
        results: 2,
        details: 0,
        focus: "Input",
        next: "Use the physical Shift key for the colon.",
    },
    Checkpoint {
        name: FOREIGN_OPERATION,
        results: 3,
        details: 0,
        focus: "Input",
        next: "Check that the old operation is rejected, then enter: shutdown.",
    },
];

impl DemoBoot {
    fn checkpoints(&self) -> &'static [Checkpoint] {
        match self {
            Self::Create => CREATE,
            Self::Reopen { .. } => REOPEN,
        }
    }
}

pub(in crate::vm_workbench) fn prompt(demo: &DemoBoot) {
    match demo {
        DemoBoot::Create => println!(
            "Cocoa window 1/2 ready. Use the physical keyboard, including Shift for the quotes. Enter: write {TARGET} \"{PAYLOAD}\". Wait for automatic-list capture."
        ),
        DemoBoot::Reopen { .. } => println!(
            "Cocoa window 2/2 ready. With the terminal input focused, enter: ls /data. Wait."
        ),
    }
}

impl Checkpoint {
    fn ready(&self, state: &BTreeMap<&str, &str>) -> Result<bool> {
        let results = number(state, "results")?;
        if results > self.results {
            return Err(bad(format!("manual checkpoint skipped: {}", self.name)));
        }
        Ok(results == self.results
            && number(state, "details")? == self.details
            && state.get("focus") == Some(&self.focus))
    }
}

impl Session {
    pub(super) fn manual_demo_poll(&mut self, serial: &str, frame: &str) -> Result<bool> {
        let demo = self.demo.as_ref().expect("demo session");
        // 第一个快照是窗口初始帧，其余快照依次对应人工检查点。
        let index = self.snapshots.len() - 1;
        let Some(step) = demo.checkpoints().get(index) else {
            return Ok(false);
        };
        let state = fields(frame)?;
        if !step.ready(&state)? {
            return Ok(false);
        }
        if matches!(demo, DemoBoot::Create) && index == 0 {
            if state.get("update") != Some(&"refreshed") {
                return Ok(false);
            }
            let (complete, _) = serial
                .rsplit_once('\n')
                .ok_or_else(|| bad("manual frame"))?;
            link::visible(complete, NAME)?;
        }
        if step.name == PREFILLED
            && let DemoBoot::Reopen { old_operation } = demo
        {
            let (prefix, suffix) = explain_input(old_operation)?;
            let sent = self.manual_prefill_bytes;
            if number(&state, "input_bytes")? != sent as u64 {
                return Err(bad("unexpected draft while prefilling manual explain"));
            }
            // 每次监视轮只发一键，让外层继续执行窗口超时和退出检查。
            if sent < prefix.len() {
                let byte = prefix
                    .get(sent..sent + 1)
                    .ok_or_else(|| bad("non-ASCII manual explain prefix"))?;
                self.type_text(byte)?;
                self.manual_prefill_bytes += 1;
                return Ok(false);
            }
            self.snapshot_at(step.name, serial, frame)?;
            let read = read_trace(&self.directory, REOPENED_BYTES)?;
            let prefilled = read_trace(&self.directory, PREFILLED)?;
            if explain_draft(since(&prefilled, &read)?)? != prefix.as_bytes() {
                return Err(bad(
                    "manual explain prefill differs from first boot identity",
                ));
            }
            println!(
                "Recorded {}. Host prefilled `{prefix}`. Type only `{suffix}` and press Enter. {}",
                step.name, step.next
            );
            return Ok(false);
        }
        self.snapshot_at(step.name, serial, frame)?;
        println!("Recorded {}. {}", step.name, step.next);
        Ok(false)
    }
}

fn explain_input(old_operation: &str) -> Result<(String, String)> {
    let (boot, sequence) = old_operation
        .split_once(':')
        .ok_or_else(|| bad("missing first boot operation suffix"))?;
    // 首窗已按分配的 boot_id 验证完整操作 ID。
    Ok((format!("explain {boot}"), format!(":{sequence}")))
}

fn explain_draft(trace: &str) -> Result<Vec<u8>> {
    let mut draft = Vec::new();
    for line in trace.lines() {
        let words = line.split_whitespace().collect::<Vec<_>>();
        match words.as_slice() {
            ["text", byte] => draft.push(byte.parse().map_err(|_| bad("invalid explain byte"))?),
            ["key", "backspace", "0" | "1", _] if draft.pop().is_some() => (),
            ["key", "other", "0" | "1", _]
            | ["key", "c", "0" | "1", "0" | "1"]
            | ["key", _, "2", _]
            | ["move", _, _]
            | ["observe"] => (),
            _ => return Err(bad("unexpected edit or focus change in manual explain")),
        }
    }
    Ok(draft)
}

fn verify_explain(prefill: &str, manual: &str, old_operation: &str) -> Result<()> {
    let (prefix, suffix) = explain_input(old_operation)?;
    if explain_draft(prefill)? != prefix.as_bytes() {
        return Err(bad(
            "manual explain prefill differs from first boot identity",
        ));
    }
    let (draft, after_enter) = manual
        .split_once("key enter 0 0\n")
        .ok_or_else(|| bad("manual explain missing Enter evidence"))?;
    if explain_draft(draft)? != suffix.as_bytes()
        || shifted_texts(draft, "text 58") == 0
        || after_enter
            .lines()
            .filter(|line| *line == "execute")
            .count()
            != 1
    {
        return Err(bad(
            "manual explain must submit the original operation with Shift colon",
        ));
    }
    Ok(())
}

fn key_press(step: &str, name: &str, shifted: bool) -> bool {
    let mut words = step.split_whitespace();
    words.next() == Some("key")
        && words.next() == Some(name)
        && words.next() == Some("0")
        && words.next().and_then(|mask| mask.parse::<u8>().ok()) == Some(u8::from(shifted))
}

fn shifted_texts(trace: &str, text: &str) -> usize {
    let events = trace
        .lines()
        .filter(|line| line.starts_with("key ") || line.starts_with("text "));
    events
        .clone()
        .zip(events.skip(1))
        .filter(|(key, value)| key_press(key, "other", true) && *value == text)
        .count()
}

fn since<'a>(trace: &'a str, previous: &str) -> Result<&'a str> {
    trace
        .strip_prefix(previous)
        .ok_or_else(|| bad("manual input trace changed between frames"))
}

fn read_trace(directory: &Path, name: &str) -> Result<String> {
    Ok(fs::read_to_string(directory.join(format!("{name}.steps")))?)
}

fn keyboard_closed_details(trace: &str) -> bool {
    let mut events = trace.lines();
    !trace.lines().any(|line| key_press(line, "escape", false))
        && events.any(|line| key_press(line, "tab", true))
        && events.any(|line| key_press(line, "enter", false))
}

pub(in crate::vm_workbench) fn verify(
    demo: &DemoBoot,
    directory: &Path,
    snapshots: &[Snapshot],
) -> Result<()> {
    if !snapshots
        .iter()
        .skip(1)
        .map(|shot| shot.name.as_str())
        .eq(demo.checkpoints().iter().map(|step| step.name))
    {
        return Err(bad("manual checkpoints incomplete at guest exit"));
    }
    let trace = |name| read_trace(directory, name);
    match demo {
        DemoBoot::Create => {
            let created = trace("demo-automatic-file")?;
            let details = trace("demo-original-write-details")?;
            let focused = trace("demo-details-tab-focus")?;
            let closed = trace("demo-details-keyboard-close")?;
            let mut mouse = since(&details, &created)?.lines();
            if shifted_texts(&created, "text 34") < 2
                || !mouse.any(|line| line == "button 1")
                || !mouse.any(|line| line == "button 0")
                || !since(&focused, &details)?
                    .lines()
                    .any(|line| key_press(line, "tab", false))
                || !keyboard_closed_details(since(&closed, &focused)?)
            {
                return Err(bad(
                    "manual creation missing Shift quotes, mouse click or Tab/Enter focus evidence",
                ));
            }
        }
        DemoBoot::Reopen { old_operation } => {
            let read = trace(REOPENED_BYTES)?;
            let prefilled = trace(PREFILLED)?;
            let rejected = trace(FOREIGN_OPERATION)?;
            verify_explain(
                since(&prefilled, &read)?,
                since(&rejected, &prefilled)?,
                old_operation,
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoints_reject_skipped_results_and_wait_for_focus() {
        let step = &CREATE[2];
        let wrong = fields("RR:T10:FRAME results=1 details=1 focus=CloseDetails").unwrap();
        let ready = fields("RR:T10:FRAME results=1 details=1 focus=DetailsUp").unwrap();
        let skipped = fields("RR:T10:FRAME results=2 details=0 focus=Input").unwrap();
        assert!(!step.ready(&wrong).unwrap());
        assert!(step.ready(&ready).unwrap());
        assert!(step.ready(&skipped).is_err());
    }

    #[test]
    fn shift_and_keyboard_close_require_guest_key_evidence() {
        let trace = "key other 0 1\ntext 34\nkey other 0 0\ntext 34\nkey other 0 1\ntext 58\n";
        assert_eq!(shifted_texts(trace, "text 34"), 1);
        assert_eq!(shifted_texts(trace, "text 58"), 1);
        assert_eq!(shifted_texts("text 34\n", "text 34"), 0);
        assert!(keyboard_closed_details("key tab 0 1\nkey enter 0 0\n"));
        assert!(!keyboard_closed_details("key tab 0 1\nkey escape 0 0\n"));
        assert!(!keyboard_closed_details("key enter 0 0\nkey tab 0 1\n"));
    }

    #[test]
    fn explain_requires_the_exact_prefill_and_a_manually_submitted_suffix() {
        let id = "8dca703f617a1c1e0a74f2fc87a11ec6:15";
        let prefix = "explain 8dca703f617a1c1e0a74f2fc87a11ec6";
        let prefill: String = prefix
            .bytes()
            .map(|byte| format!("text {byte}\n"))
            .collect();
        let manual = "key other 0 1\ntext 58\ntext 49\ntext 52\nkey backspace 0 0\ntext 53\nkey enter 0 0\nexecute\n";
        assert!(verify_explain(&prefill, manual, id).is_ok());
        assert!(verify_explain(&prefill, &manual.replace("text 53", "text 54"), id).is_err());
        assert!(
            verify_explain(
                &prefill,
                &manual.replace("key other 0 1", "key other 0 0"),
                id
            )
            .is_err()
        );
        assert!(verify_explain(&prefill, &manual.replace("key enter 0 0\n", ""), id).is_err());
        assert!(verify_explain(&format!("{prefill}text 58\n"), manual, id).is_err());
        assert!(verify_explain(&prefill.replacen("text 56", "text 57", 1), manual, id).is_err());
    }
}
