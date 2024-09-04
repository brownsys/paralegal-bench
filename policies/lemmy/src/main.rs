extern crate anyhow;

use clap::{Parser, Subcommand, ValueEnum};

use lemmy::eval_driver::{CommonArgs, GetUserVersion, SelectionArgs};

/// Runner for individual lemmy experiments
#[derive(Parser)]
struct Arguments {
    #[clap(flatten)]
    common: CommonArgs,
    #[clap(subcommand)]
    command: LemmyCommand,
}

/// How to drive the experiment
#[derive(Subcommand)]
enum LemmyCommand {
    /// Run a manual selection of properties and controllers
    Selection(#[clap(flatten)] SelectionArgs),
    /// Run a bug configuration with preprogrammed expectations for success and
    /// failure
    Bug {
        #[clap(long, short)]
        bug: Vec<GetUserVersion>,
    },
}

/// Cleans the environment if we are being run as "cargo run"
///
/// Also disables incremental computation to reduce the size of compile
/// artifacts generated during analys.
fn env_setup() {
    use std::env;
    for (k, _) in env::vars() {
        if k.starts_with("CARGO") || k.starts_with("RUSTUP") {
            env::remove_var(k)
        }
    }
    env::set_var("CARGO_INCREMENTAL", "false");
}

fn main() -> anyhow::Result<()> {
    let args: &'static Arguments = Box::leak(Box::new(Arguments::parse()));

    env_setup();

    let mut failed = false;
    match &args.command {
        LemmyCommand::Selection(selection_args) => {
            let res = selection_args.run(&args.common)?;
            failed |= !res;
        }
        LemmyCommand::Bug { bug } => {
            let bugs = bug
                .is_empty()
                .then_some(GetUserVersion::value_variants())
                .unwrap_or(bug.as_slice());
            for bug in bugs {
                failed |= !bug.to_config().run(&args.common);
            }
        }
    }

    if failed {
        std::process::exit(1);
    }
    anyhow::Ok(())
}
