extern crate anyhow;
extern crate clap;
extern crate hyperswitch;
extern crate paralegal_policy;

use anyhow::Result;

use hyperswitch::Policy;

#[derive(clap::Parser)]
struct Args {
    /// Policy to run. (defaults to all of them)
    #[clap(long, short)]
    policy: Vec<Policy>,
    #[clap(long, default_value = "case-studies/hyperswitch")]
    source_dir: std::path::PathBuf,
    #[clap(last = true)]
    extra_flow_args: Vec<String>,
}

fn main() -> Result<()> {
    let mut cmd = paralegal_policy::SPDGGenCommand::global();
    cmd.abort_after_analysis();
    cmd.external_annotations("external-annotations.toml");
    use clap::{Parser, ValueEnum};
    let args: &'static _ = Box::leak(Box::new(Args::parse()));
    cmd.get_command()
        .args(["--target", "router"])
        .args(&args.extra_flow_args);
    let dashes = args.extra_flow_args.iter().filter(|s| *s == "--").count();
    if dashes == 0 {
        cmd.get_command().arg("--");
    } else {
        assert_eq!(dashes, 1, "too many '--' in extra args");
    }
    cmd.get_command().arg("--lib");
    let result = cmd.run(&args.source_dir)?.with_context(|ctx| {
        let policies = if args.policy.is_empty() {
            Policy::value_variants()
        } else {
            args.policy.as_slice()
        };
        for p in policies {
            p.runnable()(ctx.clone())?
        }
        Ok(())
    })?;
    println!("{}", result.stats);
    assert!(result.success);
    Ok(())
}
