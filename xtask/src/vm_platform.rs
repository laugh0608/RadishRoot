use crate::{
    error::{Error, Result},
    vm::{fields, number},
};
pub fn validate(output: &str, fault: &str) -> Result<()> {
    let lines = output
        .lines()
        .filter(|s| s.starts_with("RR:T03:"))
        .collect::<Vec<_>>();
    let count = |p: &str| lines.iter().filter(|l| l.starts_with(p)).count();
    if lines.first() != Some(&"RR:T03:ENTER") || count("RR:T03:ENTER") != 1 {
        return Err(Error::new(
            "platform-sequence",
            "missing / duplicate T03 entry",
        ));
    }
    if !fault.is_empty() {
        let code = match fault {
            "t03-invalid-range" => "MEM_RANGE",
            "t03-bad-capability" => "PCI_CAPABILITY",
            "t03-dma-timeout" => "DMA_IN_FLIGHT",
            _ => return Err(Error::new("platform-case", "unknown case")),
        };
        if lines.get(1) != Some(&format!("RR:T03:INJECT case={fault}").as_str())
            || count("RR:T03:INJECT") != 1
            || count("RR:T03:READY") != 0
            || count("RR:T03:FAIL") != 1
            || lines.last() != Some(&format!("RR:T03:FAIL code={code} stage=platform").as_str())
        {
            return Err(Error::new("platform-fault", "wrong failure sequence"));
        }
        if fault == "t03-dma-timeout" {
            if count("RR:T03:QUARANTINE ") != 1 {
                return Err(Error::new("platform-dma", "missing quarantine facts"));
            }
            let f = fields(
                lines
                    .iter()
                    .find(|s| s.starts_with("RR:T03:QUARANTINE "))
                    .unwrap(),
            )?;
            if number(&f, "release_rejected")? != 1
                || number(&f, "allocation_rejected")? != 1
                || number(&f, "reused")? != 0
            {
                return Err(Error::new("platform-dma", "DMA released before quiescence"));
            }
        }
        return Ok(());
    }
    if count("RR:T03:READY") != 1
        || lines.last() != Some(&"RR:T03:READY storage_io=none")
        || count("RR:T03:FAIL") != 0
        || count("RR:T03:INJECT") != 0
        || count("RR:T03:QUARANTINE") != 0
    {
        return Err(Error::new(
            "platform-sequence",
            "unexpected terminal sequence",
        ));
    }
    for p in ["RR:T03:MEM ", "RR:T03:PCI ", "RR:T03:DMA "] {
        if count(p) != 1 {
            return Err(Error::new(
                "platform-facts",
                format!("missing / duplicate {p}"),
            ));
        }
    }
    let mem = fields(lines.iter().find(|s| s.starts_with("RR:T03:MEM ")).unwrap())?;
    if number(&mem, "pages")? == 0
        || number(&mem, "page_size")? != 4096
        || number(&mem, "heap_bytes")? != 1048576
        || mem.get("readback") != Some(&"pass")
        || mem.get("reclaim") != Some(&"none")
    {
        return Err(Error::new("platform-memory", "unexpected allocation facts"));
    }
    let pci = fields(lines.iter().find(|s| s.starts_with("RR:T03:PCI ")).unwrap())?;
    if number(&pci, "bus")? != 0
        || number(&pci, "device")? > 31
        || number(&pci, "function")? > 7
        || number(&pci, "vendor")? != 0x1af4
        || number(&pci, "id")? != 0x1042
        || number(&pci, "bus_master")? != 0
        || number(&pci, "capabilities")? < 4
        || number(&pci, "features")? & (1 << 32) == 0
        || number(&pci, "queue_size")? == 0
    {
        return Err(Error::new("platform-pci", "unexpected device facts"));
    }
    let dma = fields(lines.iter().find(|s| s.starts_with("RR:T03:DMA ")).unwrap())?;
    for (k, n) in [
        ("pages", 2),
        ("zeroed", 1),
        ("shared", 512),
        ("cancelled_before_submit", 1),
        ("released", 1),
    ] {
        if number(&dma, k)? != n {
            return Err(Error::new("platform-dma", "incomplete HAL roundtrip"));
        }
    }
    let map = output
        .lines()
        .filter(|s| s.starts_with("RR:T02:MEM "))
        .map(fields)
        .collect::<Result<Vec<_>>>()?;
    let heap = number(&mem, "heap_physical")?;
    let objects = [
        (heap, 1048576, 4096),
        (number(&mem, "test_physical")?, 8192, 16384),
        (number(&dma, "physical")?, 8192, 4096),
        (number(&dma, "bounce")?, 4096, 4096),
    ];
    for (base, length, align) in objects {
        let end = base
            .checked_add(length)
            .ok_or_else(|| Error::new("platform-range", "overflow"))?;
        if base < 1048576
            || !base.is_multiple_of(align)
            || !map.iter().any(|r| {
                number(r, "kind").ok() == Some(0)
                    && number(r, "base").is_ok_and(|b| b <= base)
                    && number(r, "base")
                        .ok()
                        .zip(number(r, "length").ok())
                        .and_then(|(b, n)| b.checked_add(n))
                        .is_some_and(|e| end <= e)
            })
        {
            return Err(Error::new(
                "platform-range",
                "allocation outside usable RAM / alignment",
            ));
        }
        if base != heap && base < heap + 1048576 && heap < end {
            return Err(Error::new("platform-range", "allocation overlaps heap"));
        }
    }
    let bars = lines
        .iter()
        .filter(|s| s.starts_with("RR:T03:BAR "))
        .map(|s| fields(s))
        .collect::<Result<Vec<_>>>()?;
    if bars.is_empty() || bars.len() > 6 || lines.len() != bars.len() + 5 {
        return Err(Error::new("platform-bar", "bad bar count"));
    }
    for bar in bars {
        let base = number(&bar, "base")?;
        let length = number(&bar, "length")?;
        if !base.is_multiple_of(4096)
            || !length.is_power_of_two()
            || length < 4096
            || bar.get("cache") != Some(&"uc")
        {
            return Err(Error::new("platform-bar", "invalid BAR geometry/cache"));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quarantine_requires_both_refusals() {
        let text = "RR:T03:ENTER\nRR:T03:INJECT case=t03-dma-timeout\nRR:T03:QUARANTINE physical=0x200000 bounce=0x210000 release_rejected=1 allocation_rejected=1 reused=0\nRR:T03:FAIL code=DMA_IN_FLIGHT stage=platform\n";
        assert!(validate(text, "t03-dma-timeout").is_ok());
        assert!(
            validate(
                &text.replace("allocation_rejected=1", "allocation_rejected=0"),
                "t03-dma-timeout"
            )
            .is_err()
        );
        assert!(validate(text, "").is_err());
    }
}
