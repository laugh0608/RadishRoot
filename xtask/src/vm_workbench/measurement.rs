//! M1 空闲工作台的 guest 输入到 present 配对和内存采样。
use super::{Ppm, Session, Snapshot, bad, cases, fields, number};
use crate::{
    boot_image::Manifest,
    error::Result,
    process::Runner,
    vm::{self, BootSpec, Machine},
    vm_fat,
};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

pub(crate) const MODE: &str = "t10-m1-measure";
const COUNT: usize = 100;
const P95_LIMIT_NS: u64 = 150_000_000;
const BEFORE_LAST: &str = "m1-input-99";
const LAST: &str = "m1-input-100";

pub(crate) fn execute(
    runner: &mut Runner,
    manifest: &Manifest,
    machine: &Machine,
    disk: &Path,
) -> Result<()> {
    let host = runner.run(Command::new("uname").arg("-smr"))?;
    let load_before = runner.run(&mut Command::new("uptime"))?;
    let before = vm_fat::sha(disk, runner)?;
    let output = vm::boot(
        runner,
        manifest,
        machine,
        BootSpec {
            boot_disk: disk,
            data_disk: None,
            readonly: true,
            mode: MODE,
        },
    )?;
    let load_after = runner.run(&mut Command::new("uptime"))?;
    if vm_fat::sha(disk, runner)? != before {
        return Err(bad("M1 measurement disk changed"));
    }
    let mut report = analyze(&output)?;
    report["host"] = json!({
        "system": host.trim(),
        "load_before": load_before.trim(),
        "load_after": load_after.trim(),
        "qemu_version": manifest.get("qemu_version")?,
        "machine": manifest.get("machine")?,
        "accelerator": "tcg",
        "guest_cpus": manifest.number("cpus")?,
        "guest_memory_mib": manifest.number("memory_mib")?,
        "disk_sha256": before,
    });
    fs::write(
        runner.dir.join("m1-measurement.json"),
        serde_json::to_vec_pretty(&report).map_err(|error| bad(error.to_string()))?,
    )?;
    let p95 = report["latency"]["p95_ns"]
        .as_u64()
        .ok_or_else(|| bad("M1 report missing P95"))?;
    if p95 > P95_LIMIT_NS {
        return Err(bad(format!(
            "M1 idle input P95 {p95} ns exceeds {P95_LIMIT_NS} ns"
        )));
    }
    runner.record(&format!(
        "m1_idle_measurement=pass samples={COUNT} p95_ns={p95} disk_sha256=unchanged"
    ))?;
    println!(
        "M1 idle input: {COUNT} samples, P95 {:.3} ms",
        p95 as f64 / 1_000_000.0
    );
    Ok(())
}

fn key_for(index: usize) -> (&'static str, u8) {
    if index.is_multiple_of(2) {
        ("a", b'a')
    } else {
        ("b", b'b')
    }
}

impl Session {
    pub(super) fn measurement_steps(&mut self) -> Result<()> {
        for index in 0..COUNT {
            let (key, _) = key_for(index);
            self.key(key, true)?;
            self.wait(|s| {
                let frame = fields(s)?;
                Ok(number(&frame, "input_bytes")? == index as u64 + 1
                    && frame.get("focus") == Some(&"Input")
                    && number(&frame, "pending")? == 0)
            })?;
            match index + 1 {
                99 => self.snapshot(BEFORE_LAST)?,
                100 => self.snapshot(LAST)?,
                _ => (),
            }
            self.key(key, false)?;
        }
        Ok(())
    }
}

pub(super) fn verify_frames(snapshots: &[Snapshot]) -> Result<()> {
    let [initial, before, after] = snapshots else {
        return Err(bad("M1 measurement snapshot count"));
    };
    for (snapshot, name, count) in [
        (initial, "workbench-initial", 0),
        (before, BEFORE_LAST, 99),
        (after, LAST, 100),
    ] {
        if snapshot.name != name || number(&fields(&snapshot.frame)?, "input_bytes")? != count {
            return Err(bad(format!("M1 measurement snapshot {name}")));
        }
    }
    let before = Ppm::read(&before.path)?;
    let after = Ppm::read(&after.path)?;
    // 两帧只比较输入行像素，排除指针、侧栏和输出区变化。
    for y in 680..701 {
        for x in 441..1008 {
            if before.pixel(x, y)? != after.pixel(x, y)? {
                return Ok(());
            }
        }
    }
    Err(bad("M1 consecutive drafts did not change visible text"))
}

fn one<'a>(output: &'a str, prefix: &str) -> Result<&'a str> {
    let mut lines = output.lines().filter(|line| line.starts_with(prefix));
    let line = lines
        .next()
        .filter(|_| lines.next().is_none())
        .ok_or_else(|| bad(format!("M1 missing or repeated {prefix}")))?;
    Ok(line)
}

fn p95(samples: &mut [u64; COUNT]) -> u64 {
    samples.sort_unstable();
    samples[(95 * COUNT).div_ceil(100) - 1]
}

fn analyze(output: &str) -> Result<Value> {
    cases::live_device(output, true)?;
    let text: Vec<_> = output
        .lines()
        .filter_map(|line| line.strip_prefix("RR:T10:STEP text "))
        .map(|byte| {
            byte.parse::<u8>()
                .map_err(|_| bad("M1 input text encoding"))
        })
        .collect::<Result<_>>()?;
    if !text.iter().copied().eq((0..COUNT).map(|i| key_for(i).1))
        || output.contains("RR:T07:RESET ")
        || output.contains("RR:T10:REPLY ")
    {
        return Err(bad("M1 idle input trace"));
    }
    let idle = output
        .lines()
        .rfind(|line| line.starts_with("RR:T07:IDLE "))
        .ok_or_else(|| bad("M1 final idle record"))?;
    let idle = fields(idle)?;
    if number(&idle, "raw_received")? != 3 * COUNT as u64
        || number(&idle, "raw_dropped")? != 0
        || number(&idle, "pending")? != 0
    {
        return Err(bad("M1 input transport counts"));
    }
    let frames = output
        .lines()
        .filter(|line| line.starts_with("RR:T10:FRAME "))
        .map(fields)
        .collect::<Result<Vec<_>>>()?;
    let last = frames
        .last()
        .ok_or_else(|| bad("M1 missing workbench frames"))?;
    if number(last, "releases")? != COUNT as u64 || number(last, "input_bytes")? != COUNT as u64 {
        return Err(bad("M1 final keyboard counts"));
    }
    let mut durations = [0; COUNT];
    let mut samples = Vec::with_capacity(COUNT);
    let mut previous = (0, 0);
    let mut queue_max = 0;
    for line in output
        .lines()
        .filter(|line| line.starts_with("RR:M1:INPUT "))
    {
        let input = fields(line)?;
        let index = samples.len();
        let generation = number(&input, "frame")?;
        let received = number(&input, "received_ns")?;
        let presented = number(&input, "presented_ns")?;
        let byte = number(&input, "byte")?;
        let seq = index as u64 + 1;
        if index >= COUNT
            || number(&input, "seq")? != seq
            || received <= previous.0
            || presented <= received
            || generation <= previous.1
            || byte != u64::from(key_for(index).1)
        {
            return Err(bad("M1 sample sequence or time order"));
        }
        let frame = frames
            .iter()
            .find(|frame| number(frame, "generation").ok() == Some(generation))
            .ok_or_else(|| bad("M1 sample missing corresponding frame"))?;
        if number(frame, "input_bytes")? != seq
            || number(frame, "pending")? != 0
            || frame.get("focus") != Some(&"Input")
            || number(frame, "releases")? != index as u64
        {
            return Err(bad("M1 sampled frame does not show its input"));
        }
        let queue = number(&input, "dma_queue_pages")?;
        if queue == 0 || number(&input, "dma_shared_pages")? != 0 {
            return Err(bad("M1 unexpected idle DMA state"));
        }
        queue_max = queue_max.max(queue);
        durations[index] = presented - received;
        samples.push(json!({
            "seq": seq, "frame": generation, "byte": byte, "received_ns": received,
            "presented_ns": presented, "latency_ns": durations[index],
            "heap_bytes": number(frame, "heap")?, "dma_queue_pages": queue,
        }));
        previous = (presented, generation);
    }
    if samples.len() != COUNT {
        return Err(bad("M1 incomplete input samples"));
    }
    let p95_ns = p95(&mut durations);
    let kernel = fields(one(output, "RR:T02:KERNEL ")?)?;
    let heap = fields(one(output, "RR:T06:MEMORY scope=heap ")?)?;
    let fb = fields(one(output, "RR:T07:SURFACE ")?)?;
    let config = fields(one(output, "RR:M1:CONFIG ")?)?;
    let heap_capacity = number(&heap, "tracked_bytes")?;
    let heap_sample_max = frames.iter().try_fold(0, |max, f| {
        Ok::<_, crate::error::Error>(max.max(number(f, "heap")?))
    })?;
    let tracked_frame_bytes = number(last, "frame_bytes")?;
    let backbuffer_pages_bytes = number(&fb, "page_bytes")?;
    let framebuffer_span = number(&fb, "pitch")?
        .checked_mul(number(&fb, "height")?)
        .ok_or_else(|| bad("M1 framebuffer span overflow"))?;
    if heap_sample_max > heap_capacity
        || tracked_frame_bytes < backbuffer_pages_bytes
        || framebuffer_span < number(&fb, "pixels_bytes")?
    {
        return Err(bad("M1 memory accounting bounds"));
    }
    Ok(json!({
        "workload": "idle_workbench_readonly_alternating_a_b",
        "latency": {
            "clock": "guest_hpet", "start": "i8042_make_received",
            "end": "framebuffer_present_complete", "percentile": "nearest_rank",
            "samples": samples, "p95_ns": p95_ns, "limit_ns": P95_LIMIT_NS,
            "min_ns": durations[0], "max_ns": durations[COUNT - 1],
        },
        "memory": {
            "kernel_reserved_bytes": number(&kernel, "length")?,
            "heap_capacity_bytes": heap_capacity,
            "heap_max_observed_at_frame_bytes": heap_sample_max,
            "tracked_frames_final_allocated_bytes": tracked_frame_bytes,
            "framebuffer_span_bytes": framebuffer_span,
            "guarded_backbuffer_allocated_bytes": backbuffer_pages_bytes,
            "dma_queue_max_observed_pages": queue_max,
            "dma_shared_observed_pages": 0,
            "workbench_inline_bytes": number(&config, "ui_inline_bytes")?,
            "workbench_result_count_limit": number(&config, "ui_result_limit")?,
            "workbench_result_budget_bytes": number(&config, "ui_result_bytes")?,
            "workbench_line_limit_bytes": number(&config, "input_limit")?,
            "ps2_queue_entry_limit": number(&config, "raw_capacity")?,
            "dynamic_ui_and_fat_temporaries": "part_of_heap_or_stack_not_attributed",
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_p95_keeps_the_highest_five_samples_outside_the_threshold() {
        let mut samples = [P95_LIMIT_NS; COUNT];
        samples[..5].fill(P95_LIMIT_NS + 1);
        assert_eq!(p95(&mut samples), P95_LIMIT_NS);
        samples[0] = P95_LIMIT_NS + 1;
        assert_eq!(p95(&mut samples), P95_LIMIT_NS + 1);
    }
}
