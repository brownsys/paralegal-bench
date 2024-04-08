extern crate anyhow;
extern crate clap;
extern crate hyperswitch;
extern crate paralegal_policy;

use std::{fs::File, path::PathBuf};

use anyhow::Result;

use hyperswitch::Policy;
use paralegal_policy::GraphLocation;

#[derive(clap::Parser)]
struct Args {
    /// Policy to run. (defaults to all of them)
    #[clap(long, short, value_enum)]
    policy: Vec<Policy>,
    #[clap(long, default_value = "case-studies/hyperswitch")]
    source_dir: std::path::PathBuf,
    #[clap(long)]
    skip_compile: bool,
    #[clap(long)]
    dump_analyzed_code: Option<PathBuf>,
    #[clap(last = true)]
    extra_flow_args: Vec<String>,
}

fn main() -> Result<()> {
    use clap::{Parser, ValueEnum};
    let args: &'static _ = Box::leak(Box::new(Args::parse()));
    let graph_loc = if args.skip_compile {
        GraphLocation::std(&args.source_dir)
    } else {
        let mut cmd = paralegal_policy::SPDGGenCommand::global();
        cmd.abort_after_analysis();
        cmd.external_annotations("external-annotations.toml");
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
        cmd.run(&args.source_dir)?
    };
    let result = graph_loc.with_context(|ctx| {
        if let Some(path) = args.dump_analyzed_code.as_ref() {
            ctx.write_analyzed_code(File::create(path)?, false)?;
        }
        let policies = if args.policy.is_empty() {
            &[Policy::CardStorage, Policy::ApikeyStorage]
            // Policy::value_variants()
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
