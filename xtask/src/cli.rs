use crate::error::{Error, Result};

#[derive(Debug, PartialEq)]
pub enum Action {
    Help,
    Check,
    Build { release: bool, fresh: bool },
    TestHost,
    Image,
    Run,
    RunGui,
    TestVm,
    TestStorageVm,
    TestFatVm,
    TestRuntimeVm,
    TestGraphicsVm,
    TestGraphicsCases(Vec<&'static str>),
    Ui { gui: bool },
}
#[derive(Debug)]
pub struct Options {
    pub action: Action,
    pub timeout: u64,
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
        ["run", "--serial"] => Action::Run,
        ["run", "--gui"] => Action::RunGui,
        ["test", "--vm"] => Action::TestVm,
        ["test", "--vm", "--storage-only"] => Action::TestStorageVm,
        ["test", "--vm", "--fat-only"] => Action::TestFatVm,
        ["test", "--vm", "--runtime-only"] => Action::TestRuntimeVm,
        ["test", "--vm", "--graphics-only"] => Action::TestGraphicsVm,
        ["test", "--vm", "--graphics-cases", selected] => {
            let mut cases = Vec::new();
            for value in selected.split(',') {
                let case = crate::vm_graphics::CASES
                    .iter()
                    .copied()
                    .find(|case| *case == value)
                    .ok_or_else(|| Error::usage("unknown T07 graphics case"))?;
                if cases.contains(&case) {
                    return Err(Error::usage("duplicate T07 graphics case"));
                }
                cases.push(case);
            }
            Action::TestGraphicsCases(cases)
        }
        ["ui", "--host"] => Action::Ui { gui: false },
        ["ui", "--host", "--gui"] => Action::Ui { gui: true },
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
                "expected check | build --profile dev|release [--fresh] | test --host | help",
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
    }
}
