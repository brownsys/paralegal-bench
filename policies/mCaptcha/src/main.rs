use anyhow::{Ok, Result};
use clap::ValueEnum;
use paralegal_policy::GraphLocation;

use mCaptcha::Policy;

#[derive(clap::Parser)]
struct Args {
    /// Policy to run. (defaults to all of them)
    #[clap(long, short, value_enum)]
    policy: Vec<Policy>,
    #[clap(long, default_value = "case-studies/mCaptcha")]
    source_dir: std::path::PathBuf,
    #[clap(long)]
    skip_compile: bool,
    #[clap(long)]
    buggy: bool,
    // #[clap(long, value_enum)]
    // controller: Vec<Controllers>,
    #[clap(last = true)]
    extra_flow_args: Vec<String>,
}

fn main() -> Result<()> {
    use clap::Parser;
    let args: &'static _ = Box::leak(Box::new(Args::parse()));
    let graph_loc = if args.skip_compile {
        GraphLocation::std(&args.source_dir)
    } else {
        let mut cmd = paralegal_policy::SPDGGenCommand::global();
        cmd.abort_after_analysis();
        cmd.external_annotations("external-annotations.toml");
        cmd.get_command()
            .args(["--target", "mcaptcha"])
            .args(&args.extra_flow_args);
        let dashes = args.extra_flow_args.iter().filter(|s| *s == "--").count();
        if dashes == 0 {
            cmd.get_command().arg("--");
        } else {
            assert_eq!(dashes, 1, "too many '--' in extra args");
        }

        if args.buggy {
            cmd.get_command().args(["--features", "buggy"]);
        }

        cmd.get_command().args(
            mCaptcha::DEFAULT_CONTROLLERS
                .iter()
                .flat_map(|c| ["--features", *c]),
        );

        // cmd.get_command().args(
        //     if args.controller.is_empty() {
        //         Controllers::value_variants()
        //     } else {
        //         args.controller.as_slice()
        //     }
        //     .iter()
        //     .flat_map(|c| ["--features", c.as_ref()]),
        // );
        cmd.run(&args.source_dir)?
    };
    let result = graph_loc.with_context(|ctx| {
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
