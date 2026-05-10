extern crate anyhow;
extern crate clap;
extern crate paralegal_policy;

use std::{ffi::OsString, fs::File, path::PathBuf, process::exit};

use atomic::DEFAULT_CONTROLLERS;
use clap::Parser;

use anyhow::Result;
use paralegal_policy::{algo::ahb, GraphLocation};

#[derive(Parser)]
struct Arguments {
    #[clap(long)]
    buggy: bool,
    #[clap(long)]
    skip_compile: bool,
    #[clap(default_value = "case-studies/atomic-server")]
    directory: PathBuf,
    #[clap(long, default_value = "external-annotations.toml")]
    annotations: PathBuf,
    #[clap(long)]
    dump_analyzed_code: Option<PathBuf>,
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
        cmd.get_command()
            .args(DEFAULT_CONTROLLERS.iter().flat_map(|c| ["--features", c]));
        cmd.run(&args.directory)?
    };

    let mut config = paralegal_policy::Config::default();
    config.always_happens_before_tracing = ahb::TraceLevel::Full;

    let result = graph_loc.with_context_configured(config, |ctx| {
        if let Some(target) = args.dump_analyzed_code.as_ref() {
            ctx.write_analyzed_code(File::create(target)?, false, false)?;
        }
        atomic::check_rights(ctx)
    })?;
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
