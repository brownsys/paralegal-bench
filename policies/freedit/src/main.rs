extern crate anyhow;
extern crate paralegal_policy;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use paralegal_policy::GraphLocation;

use freedit::Policy;

#[derive(Parser)]
struct Args {
    #[clap(long, value_enum)]
    policy: Vec<Policy>,
    #[clap(long)]
    skip_compile: bool,
    #[clap(long)]
    buggy: bool,
}

fn main() -> Result<()> {
    let dir = "..";
    let args = Args::parse();
    let graph_loc = if args.skip_compile {
        GraphLocation::std(dir)
    } else {
        let mut cmd = paralegal_policy::SPDGGenCommand::global();
        cmd.external_annotations("external-annotations.toml")
            .abort_after_analysis()
            .get_command()
            .args(["--", "--lib"]);
        if args.buggy {
            cmd.get_command().args(["--features", "buggy"]);
        }
        cmd.run(dir)?
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
            .map(|p| p.check(ctx.clone()))
            .collect::<Result<()>>()
    })?;
    println!("Policy check succeeded: {}", res.stats);
    Ok(())
}
