use anyhow::{Ok, Result};
use clap::{Parser, ValueEnum};
use contile::{Policy, DEFAULT_CONTROLLERS};
use paralegal_policy::{algo::ahb::TraceLevel, Config, GraphLocation, SPDGGenCommand};
use std::{fs::File, path::PathBuf};

#[derive(Parser)]
struct Arguments {
    #[clap(long, default_value = "case-studies/contile")]
    repo_dir: PathBuf,
    #[clap(long)]
    skip_compile: bool,
    #[clap(long)]
    dump_analyzed_code: Option<PathBuf>,
    #[clap(long, short, value_enum)]
    policy: Vec<Policy>,
    #[clap(long, conflicts_with = "skip_compile")]
    buggy: bool,
    #[clap(last = true)]
    extra_args: Vec<String>,
}

fn main() -> Result<()> {
    let args: &'static _ = Box::leak(Box::new(Arguments::parse()));

    let graph = if args.skip_compile {
        GraphLocation::std(&args.repo_dir)
    } else {
        let mut cmd = SPDGGenCommand::global();
        cmd.external_annotations("external-annotations.toml");
        cmd.get_command().args(args.extra_args.iter());
        if !args.extra_args.contains(&"--".to_owned()) {
            cmd.get_command().arg("--");
        }
        cmd.get_command().arg("--lib");
        if args.buggy {
            cmd.get_command().args(["--features", "leak"]);
        }
        cmd.get_command()
            .args(DEFAULT_CONTROLLERS.iter().flat_map(|c| ["--features", c]));
        cmd.run(&args.repo_dir)?
    };
    let mut config = Config::default();
    config.always_happens_before_tracing = TraceLevel::Full;
    let result = graph.with_context_configured(config, |ctx| {
        if let Some(path) = args.dump_analyzed_code.as_ref() {
            ctx.write_analyzed_code(File::create(path)?, false, false)?;
        }
        let policy = if args.policy.is_empty() {
            Policy::value_variants()
        } else {
            args.policy.as_slice()
        };
        for p in policy {
            p.runnable()(ctx.clone())?
        }
        Ok(())
    })?;

    assert!(result.success);
    Ok(())
}
