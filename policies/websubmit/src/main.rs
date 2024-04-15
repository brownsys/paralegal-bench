extern crate anyhow;

use anyhow::Result;
use clap::{Parser, ValueEnum};

use websubmit::*;

#[derive(Parser)]
struct Args {
    /// path to WebSubmit directory.
    #[clap(default_value = "case-studies/websubmit")]
    ws_dir: std::path::PathBuf,

    /// `<articulation point>-<short edit type>`
    #[clap(long)]
    edit_type: Option<String>,

    #[clap(long, default_value_t = Flavour::Application, value_enum)]
    flavour: Flavour,

    /// sc, del, or dis.
    #[clap(long, value_enum)]
    policy: Vec<Policy>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mut command = paralegal_policy::SPDGGenCommand::global();
    command
        .external_annotations(args.flavour.external_annotations())
        .abort_after_analysis()
        .get_command()
        .arg("--");

    let policies = if args.policy.is_empty() {
        Policy::value_variants()
    } else {
        args.policy.as_slice()
    };

    if let Some(edit) = args.edit_type.as_ref() {
        for p in policies.iter() {
            command
                .get_command()
                .args(["--features", &format!("edit-{}-{edit}", p.short_name())]);
        }
    }
    command
        .get_command()
        .args(["--features", args.flavour.annotation_feature()]);

    command.get_command().args(
        websubmit::DEFAULT_CONTROLLERS
            .iter()
            .flat_map(|c| ["--features", c]),
    );

    let mut cfg = paralegal_policy::Config::default();
    cfg.always_happens_before_tracing = paralegal_policy::algo::ahb::TraceLevel::Full;
    let res = command
        .run(args.ws_dir)?
        .with_context_configured(cfg, |ctx| {
            for prop in policies.iter() {
                prop.runnable(args.flavour)(ctx.clone())?;
            }
            Ok(())
        })?;

    println!("Statistics for policy run {}", res.stats);
    assert!(res.success);

    Ok(())
}
