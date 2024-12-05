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

#[derive(clap::Parser)]
struct Args {
    dir: PathBuf,
    #[clap(last = true)]
    flow_args: Vec<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    env_setup();
    let mut cmd = paralegal_policy::SPDGGenCommand::global();
    cmd.external_annotations("external-annotations.toml")
        .abort_after_analysis()
        .get_command()
        .args(["--relaxed", "--target", "lemmy_api"])
        .args(args.flow_args.iter());
    let result = cmd
        .run(&args.dir)?
        .with_context(lemmy_unhacked::manual::check)?;
    assert!(result.success, "Policy failed");
    println!("policy succeeded");
    Ok(())
}
