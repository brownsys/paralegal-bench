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

fn main() -> anyhow::Result<()> {
    let args: &'static Arguments = Box::leak(Box::new(Arguments::parse()));

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
