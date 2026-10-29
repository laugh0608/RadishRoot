//! T09 外部串口输入与独立期望；不调用 guest parser / renderer 生成预期。
mod console;
mod validate;
use crate::{
    boot_image::{self, Manifest},
    config::Context,
    error::{Error, Result},
    process::Runner,
    vm::{self, BootSpec, Machine},
    vm_fat::{self, Expected, FatCheck},
    vm_storage,
};
pub use console::Session;
use std::{
    fs::{self, OpenOptions},
    io::{Seek, SeekFrom, Write},
    path::Path,
};
pub use validate::{validate, validate_default};
const CASES: [&str; 7] = [
    "t09-write",
    "t09-reopen",
    "t09-readonly",
    "t09-file-sync",
    "t09-volume-sync",
    "t09-device-flush",
    "t09-bad-fat",
];
fn expected(mode: &str) -> Vec<Expected> {
    let mut files = vec![Expected {
        id: "seed",
        path: "seed.txt".into(),
        bytes: b"RadishRoot T09 seed\n".to_vec(),
    }];
    if matches!(mode, "t09-write" | "t09-reopen") {
        files.push(Expected {
            id: "note",
            path: "note one.txt".into(),
            bytes: b"hello \"quoted\"\n".to_vec(),
        });
        files.push(Expected {
            id: "unicode",
            path: "萝卜.txt".into(),
            bytes: "中文 value".as_bytes().to_vec(),
        });
    }
    if matches!(mode, "t09-volume-sync" | "t09-device-flush") {
        files.push(Expected {
            id: "failed",
            path: "failed.txt".into(),
            bytes: b"partial payload\n".to_vec(),
        });
    }
    files
}
fn tools(
    c: &Context,
    r: &mut Runner,
    m: &Manifest,
    d: &Path,
    action: &str,
    mode: &str,
) -> Result<()> {
    vm_fat::tools_with_files(
        c,
        r,
        m,
        d,
        FatCheck {
            mode: action,
            files: expected(mode),
            directories: Vec::new(),
        },
    )?;
    Ok(())
}
pub(super) fn execute(
    c: &Context,
    r: &mut Runner,
    m: &Manifest,
    machine: &Machine,
    kernel: &Path,
    source: &Path,
) -> Result<()> {
    let shared = vm_storage::clone_sparse(source, r)?;
    tools(c, r, m, &shared, "prepare", "")?;
    for mode in CASES {
        let data = if matches!(mode, "t09-write" | "t09-reopen") {
            shared.clone()
        } else {
            let d = vm_storage::clone_sparse(source, r)?;
            tools(c, r, m, &d, "prepare", "")?;
            d
        };
        if mode == "t09-bad-fat" {
            let mut f = OpenOptions::new().write(true).open(&data)?;
            for sector in [0, 6] {
                f.seek(SeekFrom::Start(
                    (m.number("data_start")? + sector) * 512 + 11,
                ))?;
                f.write_all(&[0, 0])?;
            }
            f.sync_all()?;
            fs::write(
                data.parent().unwrap().join("fat-fault.txt"),
                "T09: invalid primary/backup bytes_per_sector; no repair\n",
            )?;
        }
        let readonly = matches!(mode, "t09-reopen" | "t09-readonly" | "t09-bad-fat");
        let before = vm_fat::sha(&data, r)?;
        let boot = boot_image::create(&c.root, r, m, kernel, mode)?;
        vm::boot(
            r,
            m,
            machine,
            BootSpec {
                boot_disk: &boot,
                data_disk: Some(&data),
                readonly,
                mode,
            },
        )?;
        let after = vm_fat::sha(&data, r)?;
        if readonly && before != after {
            return Err(Error::new("shell-readonly", "disk changed"));
        }
        if mode == "t09-write" && before == after {
            return Err(Error::new("shell-write", "disk unchanged"));
        }
        match mode {
            "t09-write" | "t09-reopen" | "t09-readonly" => tools(c, r, m, &data, "verify", mode)?,
            "t09-file-sync" => tools(c, r, m, &data, "observe-file", mode)?,
            "t09-volume-sync" | "t09-device-flush" => tools(c, r, m, &data, "observe", mode)?,
            _ => (),
        }
        vm_fat::outside_unchanged(source, &data, m)?;
        r.record(&format!("shell_case={mode} actual_console_input=pass independent_DATA=checked outside_DATA=unchanged readonly={readonly} disk={}",data.display()))?;
        println!("T09 {mode}: console input, typed records and independent disk checks passed");
    }
    Ok(())
}
pub(super) fn script(mode: &str, id: [u8; 16]) -> Result<Vec<u8>> {
    let boot = format!("{:032x}", u128::from_be_bytes(id));
    let commands = match mode {
        "t09-write" => vec![
            "helx\x1b[D\x1b[3~p\r".into(),
            "ls /data\r".into(),
            "write bad\r".into(),
            "write \"broken\r".into(),
            "write /data/../escape x\r".into(),
            "write \"note one.txt\" \"hello \\\"quoted\\\"\\n\"\r".into(),
            "write \"note one.txt\" again\r".into(),
            "cat \"note one.txt\"\r".into(),
            "operations\r".into(),
            format!("explain {boot}:3\r"),
            format!("explain {boot}:4\r"),
            "write \"萝卜.txt\" \"中文 value\"\r".into(),
            "cat \"萝卜.txt\"\r".into(),
            "ls\r".into(),
            "cat seed.txt\r".into(),
            format!(
                "explain {}:3\r",
                if id == [2; 16] {
                    "03030303030303030303030303030303"
                } else {
                    "02020202020202020202020202020202"
                }
            ),
            "hel\x1b[A\x1b[Bp\r".into(),
            "cat missing.txt\r".into(),
            "cat seed.txt\r".into(),
            "ls --next\r".into(),
        ],
        "t09-reopen" => vec![
            "help\r".into(),
            "ls\r".into(),
            "cat \"note one.txt\"\r".into(),
            "cat \"萝卜.txt\"\r".into(),
            "cat seed.txt\r".into(),
            "operations\r".into(),
        ],
        "t09-readonly" => vec![
            "write denied.txt x\r".into(),
            format!("explain {boot}:1\r"),
            "cat seed.txt\r".into(),
            "operations\r".into(),
        ],
        "t09-file-sync" | "t09-volume-sync" | "t09-device-flush" => vec![
            "write failed.txt \"partial payload\\n\"\r".into(),
            format!("explain {boot}:1\r"),
            "ls\r".into(),
            "operations\r".into(),
        ],
        "t09-bad-fat" => vec![
            "ls\r".into(),
            format!("explain {boot}:1\r"),
            "ls\r".into(),
            "operations\r".into(),
        ],
        _ => return Err(Error::new("shell-case", "unknown")),
    };
    let mut bytes = Vec::new();
    for (i, c) in commands.iter().enumerate() {
        if mode == "t09-write" && i == 14 {
            bytes.extend_from_slice(b"write forbidden.txt x\xff\r");
        }
        bytes.extend_from_slice(c.as_bytes());
    }
    Ok(bytes)
}
