use crate::{
    error::{Error, Result},
    process::Runner,
};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

pub const TARGET: &str = "x86_64-unknown-none";
#[derive(Debug)]
pub struct Toolchain {
    pub channel: String,
    pub components: Vec<String>,
    pub targets: Vec<String>,
}
fn parse_string(s: &str) -> Result<String> {
    let value = s
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .filter(|s| {
            !s.is_empty()
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        })
        .ok_or_else(|| Error::new("toolchain-config", "expected simple quoted identifier"))?;
    Ok(value.into())
}
fn parse_list(s: &str) -> Result<Vec<String>> {
    let list = s
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(|| Error::new("toolchain-config", "expected single-line list"))?;
    list.split(',').map(|s| parse_string(s.trim())).collect()
}
pub fn parse(text: &str) -> Result<Toolchain> {
    let mut section = false;
    let mut values = BTreeMap::new();
    for line in text
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
    {
        if line == "[toolchain]" && !section {
            section = true;
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .filter(|_| section)
            .ok_or_else(|| Error::new("toolchain-config", "expected one [toolchain] section"))?;
        if values.insert(key.trim(), value.trim()).is_some() {
            return Err(Error::new("toolchain-config", "duplicate key"));
        }
    }
    if values.keys().copied().collect::<Vec<_>>() != ["channel", "components", "profile", "targets"]
    {
        return Err(Error::new(
            "toolchain-config",
            "expected channel/components/profile/targets only",
        ));
    }
    let channel = parse_string(values["channel"])?;
    let date = channel.strip_prefix("nightly-").unwrap_or("");
    if date.len() != 10
        || !date.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
        || parse_string(values["profile"])? != "minimal"
    {
        return Err(Error::new(
            "toolchain-config",
            "requires dated nightly and minimal profile",
        ));
    }
    let components = parse_list(values["components"])?;
    let targets = parse_list(values["targets"])?;
    if components != ["rust-src", "rustfmt", "clippy"] || targets != [TARGET] {
        return Err(Error::new(
            "toolchain-config",
            "T01 component/target contract drift",
        ));
    }
    Ok(Toolchain {
        channel,
        components,
        targets,
    })
}

pub fn conflict_key(key: &str) -> bool {
    matches!(
        key,
        "RUSTFLAGS"
            | "CARGO_ENCODED_RUSTFLAGS"
            | "RUSTDOCFLAGS"
            | "CARGO_ENCODED_RUSTDOCFLAGS"
            | "RUSTC"
            | "RUSTC_WRAPPER"
            | "RUSTC_WORKSPACE_WRAPPER"
            | "RUSTDOC"
            | "CARGO_TARGET_DIR"
            | "CARGO_BUILD_TARGET_DIR"
            | "CARGO_INCREMENTAL"
    ) || key.starts_with("CARGO_BUILD_")
        || key.starts_with("CARGO_TARGET_")
        || key.starts_with("CARGO_PROFILE_")
        || key.starts_with("CARGO_ALIAS_")
        || key.starts_with("CARGO_UNSTABLE_")
}
pub fn cargo_config_scope(text: &str) -> Result<()> {
    let mut section = "";
    let mut keys = std::collections::BTreeSet::new();
    for line in text
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
    {
        if line.starts_with('[') {
            if !matches!(line, "[alias]" | "[target.x86_64-unknown-none]") {
                return Err(Error::new(
                    "config-conflict",
                    "Cargo config section outside T01 scope",
                ));
            }
            section = line;
            continue;
        }
        let (key, _) = line
            .split_once('=')
            .ok_or_else(|| Error::new("config-conflict", "expected single-line Cargo setting"))?;
        let key = (section, key.trim());
        if !matches!(
            key,
            ("[alias]", "xtask") | ("[target.x86_64-unknown-none]", "rustflags")
        ) || !keys.insert(key)
        {
            return Err(Error::new(
                "config-conflict",
                "unexpected or duplicate Cargo setting",
            ));
        }
    }
    if keys.len() != 2 {
        return Err(Error::new("config-conflict", "missing Cargo setting"));
    }
    Ok(())
}
pub fn external_configs(root: &Path, cargo_home: &Path) -> Result<()> {
    let mut candidates = vec![
        cargo_home.join("config"),
        cargo_home.join("config.toml"),
        root.join(".cargo/config"),
    ];
    for ancestor in root.ancestors().skip(1) {
        candidates.extend([
            ancestor.join(".cargo/config"),
            ancestor.join(".cargo/config.toml"),
        ]);
    }
    for path in candidates {
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                return Err(Error::new(
                    "config-conflict",
                    format!(
                        "external Cargo configuration: {}; reconcile explicitly",
                        path.display()
                    ),
                ));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
pub fn require_installed(text: &str, required: &[String], kind: &'static str) -> Result<()> {
    for item in required {
        if !text
            .lines()
            .any(|line| line.split_whitespace().next() == Some(item.as_str()))
        {
            return Err(Error::new(
                kind,
                format!("missing installed item: {item}; automatic installation disabled"),
            ));
        }
    }
    Ok(())
}
pub fn selection(active: &str, expected: &str, source: Option<&Path>) -> Result<()> {
    if active.split_whitespace().next() != Some(expected) {
        return Err(Error::new(
            "toolchain-selection",
            "active/compiled toolchain differs from project",
        ));
    }
    if let Some(path) = source {
        let marker = format!("overridden by '{}'", path.display());
        if !active.contains(&marker) {
            return Err(Error::new(
                "toolchain-source",
                "selection must originate in repository rust-toolchain.toml; directory/default overrides rejected",
            ));
        }
    }
    Ok(())
}
pub struct Context {
    pub root: PathBuf,
    pub toolchain: String,
    pub host: String,
}
impl Context {
    pub fn preflight(root: &Path, runner: &mut Runner) -> Result<Self> {
        let keys = env::vars_os()
            .map(|(k, _)| k.to_string_lossy().into_owned())
            .filter(|k| conflict_key(k))
            .collect::<Vec<_>>();
        if !keys.is_empty() {
            return Err(Error::new(
                "config-conflict",
                format!("external keys: {}", keys.join(", ")),
            ));
        }
        let cargo_home = env::var_os("CARGO_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".cargo")
            });
        external_configs(root, &cargo_home)?;
        let config_path = root.join(".cargo/config.toml");
        if fs::symlink_metadata(&config_path)?.file_type().is_symlink() {
            return Err(Error::new(
                "config-conflict",
                "project Cargo configuration is a symlink",
            ));
        }
        let cargo_config = fs::read_to_string(&config_path)?;
        cargo_config_scope(&cargo_config)?;
        if cargo_config != include_str!("../../.cargo/config.toml") {
            return Err(Error::new(
                "config-conflict",
                "configuration changed since xtask compilation",
            ));
        }
        let specification = root.join("rust-toolchain.toml");
        if fs::symlink_metadata(&specification)?
            .file_type()
            .is_symlink()
        {
            return Err(Error::new(
                "toolchain-config",
                "toolchain file is a symlink",
            ));
        }
        let config = parse(&fs::read_to_string(&specification)?)?;
        let installed = runner.run(
            Command::new("rustup")
                .current_dir(root)
                .args(["toolchain", "list"]),
        )?;
        let provisional_host = env!("T01_HOST");
        let expected = format!("{}-{provisional_host}", config.channel);
        require_installed(
            &installed,
            std::slice::from_ref(&expected),
            "toolchain-missing",
        )?;
        let selected = runner.run(
            Command::new("rustup")
                .current_dir(root)
                .env_remove("RUSTUP_TOOLCHAIN")
                .args(["show", "active-toolchain"]),
        )?;
        selection(&selected, &expected, Some(&specification))?;
        let active = runner.run(
            Command::new("rustup")
                .current_dir(root)
                .args(["show", "active-toolchain"]),
        )?;
        selection(&active, &expected, None)?;
        let version = runner.run(
            Command::new("rustup")
                .current_dir(root)
                .args(["run", &expected, "rustc", "-Vv"]),
        )?;
        let host = version
            .lines()
            .find_map(|s| s.strip_prefix("host: "))
            .ok_or_else(|| Error::new("compiler", "missing host triple"))?
            .to_string();
        if host != provisional_host
            || !version
                .lines()
                .any(|s| s == concat!("commit-hash: ", env!("T01_COMMIT_HASH")))
        {
            return Err(Error::new(
                "toolchain-selection",
                "xtask was compiled by a different compiler; rebuild with project toolchain",
            ));
        }
        let components = runner.run(Command::new("rustup").current_dir(root).args([
            "component",
            "list",
            "--installed",
            "--toolchain",
            &expected,
        ]))?;
        let required = config
            .components
            .iter()
            .map(|s| {
                if s == "rust-src" {
                    s.clone()
                } else {
                    format!("{s}-{host}")
                }
            })
            .collect::<Vec<_>>();
        require_installed(&components, &required, "component-missing")?;
        let targets = runner.run(Command::new("rustup").current_dir(root).args([
            "target",
            "list",
            "--installed",
            "--toolchain",
            &expected,
        ]))?;
        require_installed(&targets, &config.targets, "target-missing")?;
        runner.record(&format!(
            "compiler={version}features=default; dependency identities in Cargo.lock"
        ))?;
        Ok(Self {
            root: root.into(),
            toolchain: expected,
            host,
        })
    }
    pub fn cargo(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new("rustup");
        cmd.current_dir(&self.root)
            .args(["run", &self.toolchain, "cargo"])
            .args(args);
        cmd
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn toolchain_file_is_single_source_and_strict() {
        let source = include_str!("../../rust-toolchain.toml");
        assert!(parse(source).is_ok());
        for text in [
            source.replace("minimal", "default"),
            source.replace("[toolchain]", "[other]"),
            format!("{source}channel = \"nightly\"\n"),
            source.replace("rust-src", "extra"),
            source.replace("nightly-", "stable-"),
        ] {
            assert_eq!(parse(&text).unwrap_err().kind, "toolchain-config");
        }
    }
    #[test]
    fn rejects_global_target_or_host_flags_in_project_config() {
        let source = include_str!("../../.cargo/config.toml");
        assert!(cargo_config_scope(source).is_ok());
        for extra in [
            "\n[build]\ntarget = \"x86_64-unknown-none\"\n",
            "\nlinker = \"cc\"\n",
            "\nrustflags = []\n",
        ] {
            assert!(cargo_config_scope(&format!("{source}{extra}")).is_err());
        }
    }
    #[test]
    fn rejects_missing_tools_and_wrong_selection() {
        for (kind, item) in [
            ("toolchain-missing", "nightly-missing"),
            ("target-missing", TARGET),
            ("component-missing", "rust-src"),
        ] {
            assert_eq!(
                require_installed("installed", &[item.into()], kind)
                    .unwrap_err()
                    .kind,
                kind
            );
        }
        assert!(selection("stable (default)", "nightly", None).is_err());
        assert!(
            selection(
                "nightly (directory override)",
                "nightly",
                Some(Path::new("/repo/rust-toolchain.toml"))
            )
            .is_err()
        );
    }
    #[test]
    fn rejects_external_compiler_and_config_keys() {
        for key in [
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "CARGO_BUILD_TARGET",
            "CARGO_TARGET_X86_64_UNKNOWN_NONE_LINKER",
            "RUSTC_WRAPPER",
            "CARGO_PROFILE_DEV_PANIC",
        ] {
            assert!(conflict_key(key), "{key}");
        }
        assert!(!conflict_key("RUSTUP_TOOLCHAIN"));
        let parent = env::temp_dir().canonicalize().unwrap();
        let dir = crate::process::unique_directory(&parent, "radishroot-config-test").unwrap();
        let root = dir.join("repo");
        fs::create_dir(&root).unwrap();
        let home = dir.join("cargo");
        fs::create_dir(&home).unwrap();
        fs::write(home.join("config.toml"), "synthetic config").unwrap();
        assert_eq!(
            external_configs(&root, &home).unwrap_err().kind,
            "config-conflict"
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
