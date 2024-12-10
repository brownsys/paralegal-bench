extern crate anyhow;
extern crate paralegal_policy;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use paralegal_policy::GraphLocation;

use freedit::Policy;

#[derive(Parser)]
struct Args {
    #[clap(long, default_value = "case-studies/freedit")]
    repo_dir: PathBuf,
    #[clap(long, short, value_enum)]
    policy: Vec<Policy>,
    #[clap(long)]
    skip_compile: bool,
    #[clap(long, conflicts_with = "skip_compile")]
    buggy: bool,
    #[clap(long)]
    cnl: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let graph_loc = if args.skip_compile {
        GraphLocation::std(&args.repo_dir)
    } else {
        let mut cmd = paralegal_policy::SPDGGenCommand::global();
        // --relaxed is now required here because of the use of "Lazy", who's
        // generics are instantiated as unresolvable function pointers.
        cmd.external_annotations("external-annotations.toml")
            .abort_after_analysis()
            .get_command()
            .args(["--relaxed", "--", "--lib"]);
        if args.buggy {
            cmd.get_command().args(["--features", "buggy"]);
        }
        cmd.get_command().args(
            freedit::DEFAULT_CONTROLLERS
                .iter()
                .flat_map(|c| ["--features", c]),
        );
        cmd.run(&args.repo_dir)?
    };
    let policy = if args.policy.is_empty() {
        Policy::value_variants()
    } else {
        args.policy.as_slice()
    };
    let res = graph_loc.with_context(|ctx| {
        assert!(ctx.desc().controllers.len() > 1);
        assert!(ctx
            .desc()
            .controllers
            .values()
            .all(|v| v.graph.node_count() > 50));
        policy
            .iter()
            .cloned()
            .map(|p| p.check(ctx.clone(), args.cnl))
            .collect::<Result<()>>()
    })?;
    println!("Policy check succeeded: {}", res.stats);
    Ok(())
}
