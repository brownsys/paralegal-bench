extern crate anyhow;
extern crate clap;
extern crate paralegal_policy;

use std::{ffi::OsString, path::PathBuf, process::exit};

use clap::Parser;

use anyhow::Result;
use paralegal_policy::{algo::ahb, GraphLocation};

#[derive(Parser)]
struct Arguments {
    #[clap(long)]
    buggy: bool,
    #[clap(long)]
    skip_compile: bool,
    directory: PathBuf,
    #[clap(long, default_value = "external-annotations.toml")]
    annotations: PathBuf,
    #[clap(last = true)]
    extra_args: Vec<OsString>,
}

fn main() -> Result<()> {
    let args: &'static _ = Box::leak(Box::new(Arguments::parse()));
    let graph_loc = if args.skip_compile {
        GraphLocation::std(&args.directory)
    } else {
        let mut cmd = paralegal_policy::SPDGGenCommand::global();
        cmd.external_annotations(&args.annotations)
            .abort_after_analysis();

        cmd.get_command()
            .args(["--target", "atomic_lib"])
            .args(args.extra_args.iter());
        if !args.extra_args.contains(&"--".into()) {
            cmd.get_command().arg("--");
        }
        cmd.get_command().args(["--lib", "--features", "db"]);

        if !args.buggy {
            cmd.get_command().args(["--features", "bug-fix"]);
        }
        cmd.run(&args.directory)?
    };

    let mut config = paralegal_policy::Config::default();
    config.always_happens_before_tracing = ahb::TraceLevel::Full;

    let result = graph_loc.with_context_configured(config, atomic::check_rights)?;
    println!(
        "Policy {}successful with {}",
        if result.success { "" } else { "un" },
        result.stats
    );
    if !result.success {
        exit(111);
    }
    Ok(())
}
