use crate::error::{Error, Result};

#[derive(Debug, PartialEq)]
pub enum Action {
    Help,
    Check,
    Build { release: bool, fresh: bool },
    TestHost,
    Image,
    Run,
    TestVm,
    TestStorageVm,
    TestFatVm,
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
        ["test", "--vm"] => Action::TestVm,
        ["test", "--vm", "--storage-only"] => Action::TestStorageVm,
        ["test", "--vm", "--fat-only"] => Action::TestFatVm,
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
