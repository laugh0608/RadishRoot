use crate::error::{Error, Result};

#[derive(Debug, PartialEq)]
pub enum Action {
    Help,
    Check,
    Build {
        release: bool,
        fresh: bool,
    },
    TestHost,
    Image,
    Demo {
        gui: bool,
    },
    Run,
    RunGui,
    TestVm,
    TestStorageVm,
    TestFatVm,
    TestServiceVm,
    TestShellVm,
    TestShellEquivalence {
        vm: bool,
    },
    TestWorkbenchVm,
    TestWorkbenchEquivalence,
    TestWorkbenchLayout,
    TestM1Measurement,
    TestLinkVm,
    TestPowerVm,
    TestPowerCases(Vec<&'static str>),
    TestRuntimeVm,
    TestGraphicsVm,
    TestGraphicsCases(Vec<&'static str>),
    Ui {
        gui: bool,
        disk: Option<std::path::PathBuf>,
    },
}
#[derive(Debug)]
pub struct Options {
    pub action: Action,
    pub timeout: u64,
}

fn select_cases(selected: &str, allowed: &[&'static str]) -> Result<Vec<&'static str>> {
    let mut cases = Vec::new();
    for value in selected.split(',') {
        let case = allowed
            .iter()
            .copied()
            .find(|case| *case == value)
            .ok_or_else(|| Error::usage("unknown selected VM case"))?;
        if cases.contains(&case) {
            return Err(Error::usage("duplicate selected VM case"));
        }
        cases.push(case);
    }
    Ok(cases)
}
pub fn parse(args: &[String]) -> Result<Options> {
    let mut args = args.iter().map(String::as_str).collect::<Vec<_>>();
    let mut timeout = 120;
    if let Some(i) = args.iter().position(|a| *a == "--timeout-secs") {
        timeout = args
            .get(i + 1)
            .and_then(|s| s.parse().ok())
            .filter(|n| (1..=3600).contains(n))
            .ok_or_else(|| Error::usage("--timeout-secs requires 1..3600"))?;
        args.drain(i..=i + 1);
    }
    let action = match args.as_slice() {
        [] | ["help"] | ["--help"] => Action::Help,
        ["check"] => Action::Check,
        ["test", "--host"] => Action::TestHost,
        ["image"] => Action::Image,
        ["demo"] | ["demo", "--gui"] => Action::Demo {
            gui: args.len() == 2,
        },
        ["run", "--serial"] => Action::Run,
        ["run", "--gui"] => Action::RunGui,
        ["test", "--vm"] => Action::TestVm,
        ["test", "--vm", "--storage-only"] => Action::TestStorageVm,
        ["test", "--vm", "--fat-only"] => Action::TestFatVm,
        ["test", "--vm", "--service-only"] => Action::TestServiceVm,
        ["test", "--vm", "--shell-only"] => Action::TestShellVm,
        ["test", target @ ("--host" | "--vm"), "--shell-equivalence"] => {
            Action::TestShellEquivalence {
                vm: *target == "--vm",
            }
        }
        ["test", "--vm", "--workbench-only"] => Action::TestWorkbenchVm,
        ["test", "--vm", "--workbench-equivalence"] => Action::TestWorkbenchEquivalence,
        ["test", "--vm", "--workbench-layout"] => Action::TestWorkbenchLayout,
        ["test", "--vm", "--m1-measure"] => Action::TestM1Measurement,
        ["test", "--vm", "--link-only"] => Action::TestLinkVm,
        ["test", "--vm", "--power-only"] => Action::TestPowerVm,
        ["test", "--vm", "--power-cases", selected] => {
            Action::TestPowerCases(select_cases(selected, &crate::vm_workbench::POWER_CASES)?)
        }
        ["test", "--vm", "--runtime-only"] => Action::TestRuntimeVm,
        ["test", "--vm", "--graphics-only"] => Action::TestGraphicsVm,
        ["test", "--vm", "--graphics-cases", selected] => {
            Action::TestGraphicsCases(select_cases(selected, crate::vm_graphics::CASES)?)
        }
        ["ui", "--host"] => Action::Ui {
            gui: false,
            disk: None,
        },
        ["ui", "--host", "--gui"] => Action::Ui {
            gui: true,
            disk: None,
        },
        ["ui", "--host", "--disk", path] => Action::Ui {
            gui: false,
            disk: Some((*path).into()),
        },
        ["ui", "--host", "--disk", path, "--gui"] => Action::Ui {
            gui: true,
            disk: Some((*path).into()),
        },
        ["build", "--profile", profile] | ["build", "--profile", profile, "--fresh"]
            if ["dev", "release"].contains(profile) =>
        {
            Action::Build {
                release: *profile == "release",
                fresh: args.len() == 4,
            }
        }
        _ => {
            return Err(Error::usage(
                "expected check | build --profile dev|release [--fresh] | test --host | demo [--gui] | help",
            ));
        }
    };
    Ok(Options { action, timeout })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }
    #[test]
    fn arguments_are_explicit() {
        assert_eq!(
            parse(&args("demo")).unwrap().action,
            Action::Demo { gui: false }
        );
        let manual = parse(&args("demo --gui --timeout-secs 300")).unwrap();
        assert_eq!(manual.action, Action::Demo { gui: true });
        assert_eq!(manual.timeout, 300);
        assert_eq!(
            parse(&args("test --vm --runtime-only")).unwrap().action,
            Action::TestRuntimeVm
        );
        assert_eq!(
            parse(&args("test --vm --fat-only")).unwrap().action,
            Action::TestFatVm
        );
        assert_eq!(
            parse(&args("test --vm --storage-only")).unwrap().action,
            Action::TestStorageVm
        );
        assert_eq!(
            parse(&args("build --profile release --fresh"))
                .unwrap()
                .action,
            Action::Build {
                release: true,
                fresh: true
            }
        );
        assert_eq!(
            parse(&args("test --host --timeout-secs 3"))
                .unwrap()
                .timeout,
            3
        );
        for s in [
            "run",
            "demo --gui --gui",
            "test --vm --power-cases unknown",
            "test --vm --power-cases t12-reopen",
            "test --vm --power-cases t12-sync,t12-sync",
            "test --vm --graphics-cases unknown",
            "test --vm --graphics-cases t07-redraw,t07-redraw",
            "test --vm --runtime-only --fat-only",
            "test --vm --runtime-only --storage-only",
            "build",
            "build --profile test",
            "test",
            "check extra",
            "check --timeout-secs 0",
            "check --timeout-secs 3601",
            "check --timeout-secs",
            "check --timeout-secs 2 --timeout-secs 3",
        ] {
            assert_eq!(parse(&args(s)).unwrap_err().exit, 2, "{s}");
        }
        assert_eq!(
            parse(&args("test --vm --power-cases t12-shutdown,t12-flush-fail"))
                .unwrap()
                .action,
            Action::TestPowerCases(vec!["t12-shutdown", "t12-flush-fail"])
        );
        assert_eq!(
            parse(&[
                "test".into(),
                "--vm".into(),
                "--power-cases".into(),
                String::new()
            ])
            .unwrap_err()
            .exit,
            2
        );
    }
}
