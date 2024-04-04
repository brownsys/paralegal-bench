fn main() {
    println!(
        "cargo:rustc-env=COMMIT_HASH={}",
        std::process::Command::new("git")
            .args(["log", "-n", "1", "--format=%H"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or("unknown".to_owned())
    );
}
