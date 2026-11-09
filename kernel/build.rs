use std::{env, path::PathBuf};

fn main() {
    let script = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"))
        .join("linker.ld");
    let map =
        PathBuf::from(env::var_os("OUT_DIR").expect("build output directory")).join("kernel.map");
    println!("cargo:rerun-if-changed={}", script.display());
    println!(
        "cargo:rustc-link-arg-bin=radishroot-kernel=-T{}",
        script.display()
    );
    println!(
        "cargo:rustc-link-arg-bin=radishroot-kernel=-Map={}",
        map.display()
    );
    println!("cargo:rustc-link-arg-bin=radishroot-kernel=--orphan-handling=error");
}
