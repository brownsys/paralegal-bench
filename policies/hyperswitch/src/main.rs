extern crate anyhow;
extern crate clap;
extern crate hyperswitch;
extern crate paralegal_policy;

use anyhow::Result;

use clap::ValueEnum;
use hyperswitch::{Controllers, Policy};
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
    #[clap(long, value_enum)]
    controller: Vec<Controllers>,
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
        // --relaxed is now required here because of the use of "Lazy", who's
        // generics are instantiated as unresolvable function pointers.
        cmd.get_command()
            .args(["--target", "router", "--relaxed"])
            .args(&args.extra_flow_args);
        let dashes = args.extra_flow_args.iter().filter(|s| *s == "--").count();
        if dashes == 0 {
            cmd.get_command().arg("--");
        } else {
            assert_eq!(dashes, 1, "too many '--' in extra args");
        }
        cmd.get_command().arg("--lib");

        cmd.get_command().args(
            if args.controller.is_empty() {
                Controllers::value_variants()
            } else {
                args.controller.as_slice()
            }
            .iter()
            .flat_map(|c| ["--features", c.as_ref()]),
        );
        cmd.run(&args.source_dir)?
    };
    let result = graph_loc.with_context(|ctx| {
        let edges: usize = ctx
            .desc()
            .controllers
            .values()
            .map(|p| p.graph.edge_count())
            .sum();
        let nodes: usize = ctx
            .desc()
            .controllers
            .values()
            .map(|p| p.graph.node_count())
            .sum();
        println!("Analyzing graph with {nodes} nodes and {edges} edges");
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
