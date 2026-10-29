use crate::{
    block_driver::Driver, boot::Facts, clock, heap, interrupts, pci, platform_memory,
    serial::Serial,
};
use core::{
    alloc::{GlobalAlloc, Layout},
    fmt::Write,
};
use radishroot_kernel::{
    runtime_contract::{EVENTS_PER_TURN, Queue, Wait, WaitError},
    storage::{BlockDevice, fat::validate_component},
};

pub fn initialize(serial: &mut Serial, facts: &Facts) -> Result<(), &'static str> {
    writeln!(
        serial,
        "RR:T06:ENTER mode={}",
        if facts.mode.is_empty()
            || (facts.mode.starts_with("t07-")
                || facts.mode.starts_with("t08-")
                || facts.mode.starts_with("t09-"))
        {
            "normal"
        } else {
            facts.mode
        }
    )
    .map_err(|_| "SERIAL_UNAVAILABLE")?;
    let (physical, period) = clock::initialize(facts)?;
    let (stack_start, stack_end, apic_before, apic_after) = interrupts::initialize()?;
    writeln!(serial,"RR:T06:PLATFORM clock=hpet physical={physical:#x} period_fs={period} counter_bits=64 irq=0 pit_divisor=11932 ist_start={stack_start:#x} ist_end={stack_end:#x} guard=1 apic_before={apic_before:#x} apic_after={apic_after:#x}").map_err(|_| "SERIAL_UNAVAILABLE")?;
    let start = clock::now().map_err(|e| e.code())?;
    let mut end = start;
    for _ in 0..10000 {
        end = clock::now().map_err(|e| e.code())?;
        if end > start {
            break;
        }
    }
    if end <= start || x86_64::instructions::interrupts::are_enabled() {
        return Err("CLOCK_MASKED_CHECK");
    }
    writeln!(serial, "RR:T06:MASKED start_ns={start} end_ns={end} if=0")
        .map_err(|_| "SERIAL_UNAVAILABLE")?;
    x86_64::instructions::interrupts::enable();
    Ok(())
}
#[derive(Clone, Copy)]
struct Event {
    id: u16,
    name: &'static str,
}
pub fn test(serial: &mut Serial, device: pci::Device, mode: &str) -> ! {
    assert!(cfg!(feature = "boot-test") && mode.starts_with("t06-"));
    let _ = writeln!(serial, "RR:T06:INJECT case={mode}");
    match mode {
        "t06-page-fault" | "t06-double-fault" => {
            let _ = writeln!(
                serial,
                "RR:T06:FAULT_EXPECT address={:#x} probe={:#x}",
                interrupts::FAULT_ADDRESS,
                interrupts::t06_fault_probe as *const () as u64
            );
            interrupts::inject_fault(mode == "t06-double-fault");
        }
        "t06-alloc-failure" => {
            let layout = Layout::from_size_align(2 * 1024 * 1024, 4096).expect("test layout");
            // SAFETY: 非零有效 layout；实际请求超过固定堆容量，只有 null 才进入真实分配失败入口。
            let pointer = unsafe { heap::HEAP.alloc(layout) };
            if !pointer.is_null() {
                interrupts::fatal(format_args!(
                    "RR:T06:FAIL code=ALLOC_UNEXPECTED_SUCCESS stage=inject\n"
                ));
            }
            alloc::alloc::handle_alloc_error(layout);
        }
        "t06-clock-stalled" => {
            let (before, after, config) = clock::inject_stop();
            let _ = writeln!(
                serial,
                "RR:T06:CLOCK_STOP before={before} after={after} enabled={}",
                config & 1
            );
            for _ in 0..=radishroot_kernel::runtime_contract::STALLED_READS {
                if let Err(error) = clock::now() {
                    interrupts::fatal(format_args!(
                        "RR:T06:FAIL code={} stage=clock\n",
                        error.code()
                    ));
                }
            }
            interrupts::fatal(format_args!(
                "RR:T06:FAIL code=CLOCK_TEST_NO_FAILURE stage=inject\n"
            ));
        }
        "t06-timeout" => {
            let mut disk = Driver::new(device, false).expect("timeout device");
            disk.inject_deadline_timeout();
            let _ = disk.read_blocks(0, &mut [0u8; 512]);
            interrupts::fatal(format_args!(
                "RR:T06:FAIL code=TIMEOUT_NOT_TRIGGERED stage=inject\n"
            ));
        }
        "t06-overflow" => run(serial, mode, None, None, None),
        "t06-invalid-input" => {
            let disk = Driver::new(device, false).expect("input control device");
            run(serial, mode, Some(disk), None, None);
        }
        _ => interrupts::fatal(format_args!("RR:T06:FAIL code=UNKNOWN_TEST stage=inject\n")),
    }
}
fn now() -> u64 {
    clock::now().unwrap_or_else(|e| {
        interrupts::fatal(format_args!("RR:T06:FAIL code={} stage=clock\n", e.code()))
    })
}
pub fn run(
    serial: &mut Serial,
    mode: &str,
    mut disk: Option<Driver>,
    facts: Option<&Facts>,
    mut files: Option<&mut crate::file_runtime::Service<'_>>,
) -> ! {
    if let Some(files) = files.as_deref() {
        let _ = writeln!(
            serial,
            "RR:T08:AVAILABLE generation={} state={:?}",
            files.generation(),
            files.volume().mount_state
        );
    }
    let mut console = crate::console::Console::new();
    if let Some(files) = files.as_deref() {
        console.announce(serial, files).unwrap_or_else(|code| {
            interrupts::fatal(format_args!("RR:T09:FAIL code={code} stage=console-init\n"))
        });
    }
    let mut graphics = None;
    let mut events = Queue::new();
    if mode == "t06-overflow" {
        for id in 0..65 {
            let _ = events.push(Event { id, name: "probe" });
        }
    } else if mode == "t06-invalid-input" {
        events
            .push(Event {
                id: 0,
                name: "bad/name",
            })
            .unwrap_or_else(|_| unreachable!());
        events
            .push(Event {
                id: 1,
                name: "valid.txt",
            })
            .unwrap_or_else(|_| unreachable!());
    }
    // 真实 IRQ 累积多个唤醒只保留一个 pending；独立记录 tick 差，不能伪造队列溢出计数。
    interrupts::take_wake();
    let ticks_before = interrupts::ticks().0;
    let mut merge_wait = Wait::new(now(), 100_000_000, 2_000_000).expect("merge wait");
    while interrupts::ticks().0.saturating_sub(ticks_before) < 2 {
        if let Err(e) = merge_wait.poll(clock::now()) {
            let (mask, irr, isr) = interrupts::diagnostic();
            let _ = writeln!(
                serial,
                "RR:T06:IRQ_DIAGNOSTIC ticks={} if={} mask={mask:#x} irr={irr:#x} isr={isr:#x} elapsed_ns={}",
                interrupts::ticks().0,
                u8::from(x86_64::instructions::interrupts::are_enabled()),
                merge_wait.elapsed()
            );
            interrupts::fatal(format_args!(
                "RR:T06:FAIL code={} stage=irq-progress\n",
                e.code()
            ));
        }
        core::hint::spin_loop();
    }
    let merge_ticks = interrupts::ticks().0 - ticks_before;
    let (one, two) = x86_64::instructions::interrupts::without_interrupts(|| {
        (interrupts::take_wake(), interrupts::take_wake())
    });
    if !one || two {
        interrupts::fatal(format_args!("RR:T06:FAIL code=WAKE_MERGE stage=events\n"));
    }
    let _ = writeln!(
        serial,
        "RR:T06:MERGE ticks={merge_ticks} consumed=1 remaining=0"
    );
    let start = now();
    let mut last = start;
    let mut wakes = 0u64;
    let mut processed = 0u64;
    let mut rejected = 0u64;
    let mut accepted = 0u64;
    let mut ordered = true;
    let mut reported = false;
    loop {
        if interrupts::take_wake() {
            wakes = wakes.saturating_add(1);
        }
        let current = now();
        if current < last {
            interrupts::fatal(format_args!(
                "RR:T06:FAIL code=CLOCK_BACKWARDS stage=loop\n"
            ));
        }
        last = current;
        for _ in 0..EVENTS_PER_TURN {
            let Some(event) = events.pop() else {
                break;
            };
            ordered &= u64::from(event.id) == processed;
            processed += 1;
            if validate_component(event.name).is_err() {
                rejected += 1;
            } else {
                accepted += 1;
            }
        }
        if wakes >= 100 && !reported {
            if let Some(mut d) = disk.take() {
                let requests = d.requests;
                if requests != 0 || rejected != 1 || accepted != 1 {
                    interrupts::fatal(format_args!(
                        "RR:T06:FAIL code=INPUT_CONTROL stage=events\n"
                    ));
                }
                if d.stop().is_err() {
                    interrupts::fatal(format_args!(
                        "RR:T06:FAIL code=INPUT_DEVICE_STOP stage=events\n"
                    ));
                }
                let _ = writeln!(
                    serial,
                    "RR:T06:INPUT invalid={rejected} valid={accepted} requests_before=0 requests_after={requests} continued=1"
                );
            }
            let (drops, saturated) = events.rejected();
            if mode == "t06-overflow" && (processed != 64 || drops != 1 || !ordered || saturated) {
                interrupts::fatal(format_args!(
                    "RR:T06:FAIL code=QUEUE_CONTRACT stage=events\n"
                ));
            }
            let _ = writeln!(
                serial,
                "RR:T06:EVENTS processed={processed} accepted={accepted} invalid={rejected} dropped={drops} saturated={} ordered={} kind=name-validation",
                u8::from(saturated),
                u8::from(ordered)
            );
            for (scope, stats) in [
                ("usable-frames", platform_memory::stats()),
                ("heap", heap::stats()),
            ] {
                let _ = writeln!(
                    serial,
                    "RR:T06:MEMORY scope={scope} total_bytes={} tracked_bytes={} allocated_bytes={} tracked_free={} untracked={} whole_machine=unknown",
                    stats.total_bytes,
                    stats.tracked_bytes,
                    stats.allocated_bytes,
                    stats.tracked_free(),
                    stats.untracked()
                );
            }
            let (ticks, saturated) = interrupts::ticks();
            if saturated || current <= start {
                interrupts::fatal(format_args!("RR:T06:FAIL code=LOOP_PROGRESS stage=loop\n"));
            }
            let _ = writeln!(
                serial,
                "RR:T06:PROGRESS wakes={wakes} ticks={ticks} start_ns={start} end_ns={current} if={}",
                u8::from(x86_64::instructions::interrupts::are_enabled())
            );
            let _ = writeln!(
                serial,
                "RR:T06:READY mode={}",
                if mode.is_empty() { "normal" } else { mode }
            );
            reported = true;
            if let Some(facts) = facts {
                graphics = Some(
                    crate::graphics::Graphics::new(serial, facts).unwrap_or_else(|code| {
                        interrupts::fatal(format_args!(
                            "RR:T07:FAIL code={code} stage=initialize\n"
                        ))
                    }),
                );
            }
        }
        let input_pending = graphics.as_mut().is_some_and(|g| {
            g.turn(serial).unwrap_or_else(|code| {
                interrupts::fatal(format_args!("RR:T07:FAIL code={code} stage=loop\n"))
            })
        });
        let console_pending = if let Some(files) = files.as_deref_mut() {
            console.turn(serial, files, now()).unwrap_or_else(|code| {
                interrupts::fatal(format_args!("RR:T09:FAIL code={code} stage=console\n"))
            })
        } else {
            false
        };
        interrupts::idle(!events.is_empty() || input_pending || console_pending);
    }
}
pub fn wait_error(error: WaitError, elapsed: u64, stage: &str) {
    if let Ok(mut serial) = Serial::new() {
        let _ = writeln!(
            serial,
            "RR:T06:WAIT reason={} elapsed_ns={elapsed} stage={stage}",
            error.code()
        );
    }
}
