use std::{env, path::Path, process::Command};

fn main() {
    eprintln!("{}", env::current_dir().unwrap().display());

    let base_dir = Path::new("../..");

    let policies = ["deletion", "verify-before-save"];
    for policy in policies {
        let mut p = Path::new("policies/mCaptcha").join(policy);
        p.set_extension("txt");
        println!("cargo:rerun-if-changed={}", base_dir.join(&p).display());
        let out_dir = env::var_os("OUT_DIR").unwrap();
        let mut out_file = Path::new(&out_dir).join(policy);
        out_file.set_extension("rs");
        let status = Command::new("cargo")
            .args(["run", "-p", "paralegal-compiler", "--"])
            .arg(p)
            .arg("-o")
            .arg(out_file)
            .current_dir(base_dir.join("paralegal-compiler"))
            .status()
            .unwrap();
        assert!(status.success());
    }
}
