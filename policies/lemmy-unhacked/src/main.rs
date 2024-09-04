use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

/// Cleans the environment if we are being run as "cargo run"
///
/// Also disables incremental computation to reduce the size of compile
/// artifacts generated during analysis.
fn env_setup() {
    use std::env;
    for (k, _) in env::vars() {
        if k.starts_with("CARGO") || k.starts_with("RUSTUP") {
            env::remove_var(k)
        }
    }
    env::set_var("CARGO_INCREMENTAL", "false");
}

//const TARGET: &str = "<lemmy_api_common::comment::SaveComment as lemmy_api::Perform>::perform";
const TARGET: &str = "<lemmy_api_common::person::Login as lemmy_api::Perform>::perform";

#[derive(clap::Parser)]
struct Args {
    dir: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    env_setup();
    let mut cmd = paralegal_policy::SPDGGenCommand::global();
    cmd.external_annotations("external-annotations.toml")
        .get_command()
        .args(["--relaxed", "--analyze", TARGET, "--target", "lemmy_api"]);
    cmd.run(&args.dir)?.with_context(lemmy_unhacked::check)?;
    println!("Policy successful");
    Ok(())
}
