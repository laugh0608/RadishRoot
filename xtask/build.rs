use std::{env, process::Command};

fn main() {
    let output = Command::new(env::var_os("RUSTC").expect("Cargo compiler"))
        .arg("-Vv")
        .output()
        .expect("compiler identity");
    assert!(output.status.success(), "compiler identity failed");
    let identity = String::from_utf8(output.stdout).expect("UTF-8 identity");
    for field in ["commit-hash", "host"] {
        let value = identity
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{field}: ")))
            .expect("compiler identity field");
        println!(
            "cargo:rustc-env=T01_{}={value}",
            field.replace('-', "_").to_uppercase()
        );
    }
    println!("cargo:rerun-if-changed=build.rs");
}
