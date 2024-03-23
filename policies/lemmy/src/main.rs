extern crate anyhow;

use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use lemmy::Prop;

#[derive(Parser)]
struct Arguments {
    path: PathBuf,
    #[clap(long)]
    skip_compile: bool,
    /// Property selection. If none are selected all are run
    #[clap(long)]
    prop: Vec<Prop>,
    #[clap(long, short)]
    quiet: bool,
    #[clap(last = true)]
    extra_args: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    let args: &'static Arguments = Box::leak(Box::new(Arguments::parse()));

    let graph_file = if args.skip_compile {
        paralegal_policy::GraphLocation::std(&args.path)
    } else {
        let mut cmd = paralegal_policy::SPDGGenCommand::global();
        cmd.external_annotations("external-annotations.toml");
        cmd.abort_after_analysis();
        cmd.get_command().arg("--target").arg("lemmy_api");
        cmd.get_command().args(&args.extra_args);
        cmd.run(&args.path)?
    };

    let res = graph_file.with_context(|cx| {
        let num_controllers = cx.desc().controllers.len();
        let sum_nodes = cx
            .desc()
            .controllers
            .values()
            .map(|spdg| spdg.graph.node_count())
            .sum::<usize>();
        println!(
            "Analyzing over {num_controllers} controllers with avg {} nodes per graph",
            sum_nodes / num_controllers
        );
        for ctrl in cx.desc().controllers.values() {
            let num_nodes = ctrl.graph.node_count();
            if num_nodes < 999 {
                println!(
                    "{} has only {num_nodes} nodes",
                    paralegal_policy::paralegal_spdg::DisplayPath::from(&ctrl.path)
                );
            }
        }
        for p in if args.prop.is_empty() {
            Prop::value_variants()
        } else {
            args.prop.as_slice()
        } {
            p.run(cx.clone())?;
        }

        anyhow::Ok(())
    })?;

    println!("Policy finished. Stats {}", res.stats);
    if !res.success {
        std::process::exit(0);
    }
    anyhow::Ok(())
}
