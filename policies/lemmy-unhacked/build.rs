use std::{env, path::Path, process::Command};

fn main() {
    let policy_file = "policy.txt";
    println!("cargo:rerun-if-changed={policy_file}");
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("policy.rs");

    let stat = Command::new("paralegal-compiler")
        .args([policy_file, "-o"])
        .arg(dest_path)
        .status()
        .unwrap();

    assert!(stat.success());
}
