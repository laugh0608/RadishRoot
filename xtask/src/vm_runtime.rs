use crate::{
    boot_image::{self, Manifest},
    config::Context,
    error::{Error, Result},
    process::Runner,
    vm::{self, BootSpec, Machine, fields, number},
};
use std::path::Path;
pub(super) fn execute(
    context: &Context,
    runner: &mut Runner,
    manifest: &Manifest,
    machine: &Machine,
    kernel: &Path,
    normal: &Path,
) -> Result<()> {
    for mode in [
        "t06-page-fault",
        "t06-double-fault",
        "t06-alloc-failure",
        "t06-overflow",
        "t06-clock-stalled",
        "t06-invalid-input",
        "t06-timeout",
        "t06-no-hpet",
    ] {
        let boot = boot_image::create(&context.root, runner, manifest, kernel, mode)?;
        let data = if mode == "t06-timeout" {
            Some(crate::vm_storage::clone_sparse(normal, runner)?)
        } else {
            None
        };
        vm::boot(
            runner,
            manifest,
            machine,
            BootSpec {
                boot_disk: &boot,
                data_disk: data.as_deref(),
                readonly: true,
                mode,
            },
        )?;
        if let Some(data) = data {
            crate::vm_storage::compare_unchanged(normal, &data)?;
            crate::digest(&data, runner)?;
        }
        println!("runtime {mode}: verified");
    }
    Ok(())
}
fn bad(detail: &str) -> Error {
    Error::new("runtime-contract", detail)
}
fn one<'a>(output: &'a str, prefix: &str) -> Result<&'a str> {
    let mut lines = output.lines().filter(|s| s.starts_with(prefix));
    let line = lines
        .next()
        .ok_or_else(|| bad(&format!("missing {prefix}")))?;
    if lines.next().is_some() {
        return Err(bad(&format!("duplicate {prefix}")));
    }
    Ok(line)
}
pub fn validate(output: &str, mode: &str) -> Result<()> {
    let label = if mode.is_empty() { "normal" } else { mode };
    if one(output, "RR:T06:ENTER ")? != format!("RR:T06:ENTER mode={label}") {
        return Err(bad("case identity"));
    }
    if output.lines().any(|l| {
        l.starts_with("RR:T02:FAIL")
            || l.starts_with("RR:T03:FAIL")
            || l.starts_with("RR:T05:FAIL")
            || l.starts_with("RR:T06:REENTRANT")
    }) {
        return Err(bad("unexpected earlier / reentrant failure"));
    }
    if mode != "t06-timeout" && output.contains("RR:T04:FAIL") {
        return Err(bad("unexpected block failure"));
    }
    let expected = match mode {
        "t06-page-fault" => Some(("PAGE_FAULT", "exception")),
        "t06-double-fault" => Some(("DOUBLE_FAULT", "exception")),
        "t06-alloc-failure" => Some(("ALLOC_FAILURE", "allocation")),
        "t06-clock-stalled" => Some(("CLOCK_STALLED", "clock")),
        "t06-timeout" => Some(("BLOCK_TIMEOUT", "request")),
        "t06-no-hpet" => Some(("HPET_MISSING", "initialize")),
        "" | "t06-overflow" | "t06-invalid-input" => None,
        _ => return Err(bad("unknown mode")),
    };
    let lines: Vec<_> = output
        .lines()
        .filter(|l| l.starts_with("RR:T06:"))
        .collect();
    if let Some((code, stage)) = expected {
        let fail = one(output, "RR:T06:FAIL ")?;
        if fail != format!("RR:T06:FAIL code={code} stage={stage}")
            || lines.last() != Some(&fail)
            || output.contains("RR:T06:READY")
        {
            return Err(bad("wrong fatal terminal"));
        }
    } else {
        let ready = one(output, "RR:T06:READY ")?;
        if ready != format!("RR:T06:READY mode={label}")
            || lines.last() != Some(&ready)
            || output.contains("RR:T06:FAIL")
        {
            return Err(bad("wrong normal terminal"));
        }
    }
    if mode == "t06-no-hpet" {
        if lines.len() != 2 {
            return Err(bad("missing clock entered runtime"));
        }
        return Ok(());
    }
    let p = fields(one(output, "RR:T06:PLATFORM ")?)?;
    if p.get("clock") != Some(&"hpet")
        || number(&p, "counter_bits")? != 64
        || number(&p, "irq")? != 0
        || number(&p, "pit_divisor")? != 11932
        || number(&p, "guard")? != 1
        || !(1..=100_000_000).contains(&number(&p, "period_fs")?)
        || number(&p, "ist_end")?.checked_sub(number(&p, "ist_start")?) != Some(16384)
    {
        return Err(bad("platform facts"));
    }
    if number(&p, "apic_after")? != number(&p, "apic_before")? & !(1 << 11)
        || number(&p, "apic_after")? & ((1 << 10) | (1 << 11)) != 0
    {
        return Err(bad("PIC mode not established"));
    }
    let masked = fields(one(output, "RR:T06:MASKED ")?)?;
    if number(&masked, "if")? != 0 || number(&masked, "end_ns")? <= number(&masked, "start_ns")? {
        return Err(bad("clock depends on IRQ"));
    }
    if !mode.is_empty() && one(output, "RR:T06:INJECT ")? != format!("RR:T06:INJECT case={mode}") {
        return Err(bad("injection identity"));
    }
    if mode.is_empty() && output.contains("RR:T06:INJECT") {
        return Err(bad("injection in normal image"));
    }
    match mode {
        "t06-page-fault" | "t06-double-fault" => {
            let e = fields(one(output, "RR:T06:EXCEPTION ")?)?;
            let expect = fields(one(output, "RR:T06:FAULT_EXPECT ")?)?;
            let vector = if mode == "t06-page-fault" { 14 } else { 8 };
            let code = if vector == 14 {
                "PAGE_FAULT"
            } else {
                "DOUBLE_FAULT"
            };
            if number(&e, "vector")? != vector
                || e.get("code") != Some(&code)
                || number(&e, "has_error")? != 1
                || number(&e, "error")? != 0
                || number(&expect, "address")? != 0xffffd00000000000
                || number(&e, "cr2")? != number(&expect, "address")?
            {
                return Err(bad("exception identity/address"));
            }
            if number(&e, "rsp")? == 0
                || number(&e, "rip")? == 0
                || number(&e, "handler_rsp")? == 0
                || number(&e, "rflags")? & 2 == 0
                || number(&e, "ist_start")? != number(&p, "ist_start")?
                || number(&e, "ist_end")? != number(&p, "ist_end")?
            {
                return Err(bad("incomplete exception context"));
            }
            if vector == 14
                && !(number(&expect, "probe")?
                    ..number(&expect, "probe")?
                        .checked_add(64)
                        .ok_or_else(|| bad("probe overflow"))?)
                    .contains(&number(&e, "rip")?)
            {
                return Err(bad("fault RIP outside probe"));
            }
            if vector == 8
                && !(number(&p, "ist_start")?..number(&p, "ist_end")?)
                    .contains(&number(&e, "handler_rsp")?)
            {
                return Err(bad("double fault did not use IST"));
            }
        }
        "t06-alloc-failure" => {
            let a = fields(one(output, "RR:T06:ALLOCATION ")?)?;
            if number(&a, "size")? != 2097152
                || number(&a, "align")? != 4096
                || number(&a, "last_heap_used")? > 1048576
                || a.get("snapshot") != Some(&"last-committed")
            {
                return Err(bad("allocation facts"));
            }
        }
        "t06-timeout" => {
            validate_wait(output, "BLOCK_TIMEOUT")?;
            let q = fields(one(output, "RR:T04:QUARANTINE ")?)?;
            if number(&q, "shares")? != 3
                || number(&q, "allocations")? == 0
                || number(&q, "release_rejected")? != 1
                || number(&q, "allocation_rejected")? != 1
                || number(&q, "reused")? != 0
                || output.contains("RR:T04:STOP")
            {
                return Err(bad("DMA ownership lost"));
            }
            let block = fields(one(output, "RR:T04:FAIL ")?)?;
            if block.get("code") != Some(&"BLOCK_TIMEOUT")
                || block.get("stage") != Some(&"Read")
                || number(&block, "sector")? != 0
                || number(&block, "completed_bytes")? != 0
            {
                return Err(bad("timeout request facts"));
            }
        }
        "t06-clock-stalled" => {
            let stopped = fields(one(output, "RR:T06:CLOCK_STOP ")?)?;
            if number(&stopped, "enabled")? != 0
                || number(&stopped, "before")? != number(&stopped, "after")?
            {
                return Err(bad("HPET did not actually stop"));
            }
        }
        _ => validate_progress(output, mode)?,
    }
    Ok(())
}
pub(super) fn validate_wait(output: &str, code: &str) -> Result<()> {
    let w = fields(one(output, "RR:T06:WAIT ")?)?;
    let reason = w.get("reason").copied().ok_or_else(|| bad("wait reason"))?;
    let elapsed = number(&w, "elapsed_ns")?;
    if w.get("stage") != Some(&"request")
        || !match (reason, code) {
            ("DEADLINE", "BLOCK_TIMEOUT") => elapsed >= 2_000_000_000,
            ("POLL_BUDGET_EXHAUSTED", "BLOCK_POLL_BUDGET") => elapsed < 2_000_000_000,
            _ => false,
        }
    {
        return Err(bad("wait cause mislabeled"));
    }
    Ok(())
}
fn validate_progress(output: &str, mode: &str) -> Result<()> {
    let merge = fields(one(output, "RR:T06:MERGE ")?)?;
    if number(&merge, "ticks")? < 2
        || number(&merge, "consumed")? != 1
        || number(&merge, "remaining")? != 0
    {
        return Err(bad("wake merge"));
    }
    let progress = fields(one(output, "RR:T06:PROGRESS ")?)?;
    if number(&progress, "wakes")? < 100
        || number(&progress, "ticks")? < number(&progress, "wakes")?
        || number(&progress, "end_ns")? <= number(&progress, "start_ns")?
        || number(&progress, "if")? != 1
    {
        return Err(bad("no real loop progress"));
    }
    let e = fields(one(output, "RR:T06:EVENTS ")?)?;
    let expected = match mode {
        "t06-overflow" => (64, 64, 0, 1),
        "t06-invalid-input" => (2, 1, 1, 0),
        _ => (0, 0, 0, 0),
    };
    if (
        number(&e, "processed")?,
        number(&e, "accepted")?,
        number(&e, "invalid")?,
        number(&e, "dropped")?,
    ) != expected
        || number(&e, "saturated")? != 0
        || number(&e, "ordered")? != 1
        || e.get("kind") != Some(&"name-validation")
    {
        return Err(bad("event order/count"));
    }
    if mode == "t06-invalid-input" {
        let i = fields(one(output, "RR:T06:INPUT ")?)?;
        for (key, value) in [
            ("invalid", 1),
            ("valid", 1),
            ("requests_before", 0),
            ("requests_after", 0),
            ("continued", 1),
        ] {
            if number(&i, key)? != value {
                return Err(bad("ordinary input stopped system or submitted IO"));
            }
        }
    }
    let memory: Vec<_> = output
        .lines()
        .filter(|l| l.starts_with("RR:T06:MEMORY "))
        .collect();
    if memory.len() != 2 {
        return Err(bad("missing memory scopes"));
    }
    let mut total = 0u64;
    let mut tracked = 0u64;
    for line in output.lines().filter(|l| l.starts_with("RR:T02:MEM ")) {
        let f = fields(line)?;
        if number(&f, "kind")? == 0 {
            let base = number(&f, "base")?;
            let length = number(&f, "length")?;
            total = total
                .checked_add(length)
                .ok_or_else(|| bad("memory sum overflow"))?;
            let start = base
                .max(1048576)
                .checked_add(4095)
                .ok_or_else(|| bad("memory start overflow"))?
                / 4096;
            let end = base
                .checked_add(length)
                .ok_or_else(|| bad("memory end overflow"))?
                .min(512 * 1024 * 1024)
                / 4096;
            tracked = tracked
                .checked_add(end.saturating_sub(start) * 4096)
                .ok_or_else(|| bad("tracked overflow"))?;
        }
    }
    for (line, scope, t, tr) in [
        (memory[0], "usable-frames", total, tracked),
        (memory[1], "heap", 1048576, 1048576),
    ] {
        let f = fields(line)?;
        let allocated = number(&f, "allocated_bytes")?;
        if f.get("scope") != Some(&scope)
            || f.get("whole_machine") != Some(&"unknown")
            || number(&f, "total_bytes")? != t
            || number(&f, "tracked_bytes")? != tr
            || allocated > tr
            || number(&f, "tracked_free")? != tr - allocated
            || number(&f, "untracked")? != t - tr
        {
            return Err(bad("memory denominator/free/unknown"));
        }
        if scope == "usable-frames" && (allocated < 1048576 || !allocated.is_multiple_of(4096)) {
            return Err(bad("missing permanent frame allocations"));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_hpet_rejects_false_success_and_duplicate_failure() {
        let good =
            "RR:T06:ENTER mode=t06-no-hpet\nRR:T06:FAIL code=HPET_MISSING stage=initialize\n";
        assert!(validate(good, "t06-no-hpet").is_ok());
        for bad in [
            good.replace("HPET_MISSING", "CLOCK_PERIOD"),
            format!("{good}RR:T06:READY mode=t06-no-hpet\n"),
            format!("{good}RR:T06:FAIL code=HPET_MISSING stage=initialize\n"),
            good.replace("RR:T06:FAIL", "RR:T06:DETAIL"),
        ] {
            assert!(validate(&bad, "t06-no-hpet").is_err());
        }
    }
    #[test]
    fn wait_proof_requires_time_and_matching_reason() {
        let good = "RR:T06:WAIT reason=DEADLINE elapsed_ns=2000000000 stage=request\n";
        assert!(validate_wait(good, "BLOCK_TIMEOUT").is_ok());
        assert!(validate_wait(&good.replace("2000000000", "1999999999"), "BLOCK_TIMEOUT").is_err());
        assert!(validate_wait(good, "BLOCK_POLL_BUDGET").is_err());
        assert!(
            validate_wait(
                "RR:T06:WAIT reason=POLL_BUDGET_EXHAUSTED elapsed_ns=123 stage=request\n",
                "BLOCK_POLL_BUDGET"
            )
            .is_ok()
        );
    }
}
