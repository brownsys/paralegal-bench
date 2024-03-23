extern crate anyhow;
extern crate clap;
extern crate paralegal_policy;

use clap::Parser;

use anyhow::Result;
use paralegal_policy::GraphLocation;

#[derive(Parser)]
struct Arguments {
    #[clap(long)]
    buggy: bool,
    #[clap(long)]
    skip_compile: bool,
    #[clap(long, default_value = "..")]
    directory: std::path::PathBuf,
}

fn main() -> Result<()> {
    let dir = "../";
    let args: &'static _ = Box::leak(Box::new(Arguments::parse()));
    let graph_loc = if args.skip_compile {
        GraphLocation::std(dir)
    } else {
        let mut cmd = paralegal_policy::SPDGGenCommand::global();
        cmd.external_annotations("external-annotations.toml")
            .abort_after_analysis();

        cmd.get_command()
            .args(["--target", "atomic_lib", "--", "--lib", "--features", "db"]);

        if !args.buggy {
            cmd.get_command().args(["--features", "bug-fix"]);
        }
        cmd.run(dir)?
    };

    let result = graph_loc.with_context(atomic::check_rights)?;
    println!(
        "Policy {}successful with {}",
        if result.success { "" } else { "un" },
        result.stats
    );
    Ok(())
}
