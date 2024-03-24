extern crate anyhow;

use anyhow::Result;
use clap::Parser;

use websubmit::*;

#[derive(Parser)]
struct Args {
    /// path to WebSubmit directory.
    ws_dir: std::path::PathBuf,

    /// `edit-<property>-<articulation point>-<short edit type>`
    #[clap(long)]
    edit_type: Option<String>,

    /// sc, del, or dis.
    #[clap(long, value_enum, default_value_t = Policy::All)]
    policy: Policy,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let prop = args.policy.runnable();

    let mut command = paralegal_policy::SPDGGenCommand::global();
    command.external_annotations("baseline-external-annotations.toml");
    command.abort_after_analysis();

    if let Some(edit) = args.edit_type.as_ref() {
        command.get_command().args(["--", "--features", &edit]);
    }
    let mut cfg = paralegal_policy::Config::default();
    cfg.always_happens_before_tracing = paralegal_policy::algo::ahb::TraceLevel::Full;
    let res = command
        .run(args.ws_dir)?
        .with_context_configured(cfg, prop)?;

    println!("Statistics for policy run {}", res.stats);
    assert!(res.success);

    Ok(())
}
