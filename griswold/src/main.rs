use anyhow::{ensure, Context};
use clap::Parser;
use input::EvaluationConfig;
use run::Output;
use std::{
    path::{Path, PathBuf},
    process,
};
use tracing::{info, level_filters::LevelFilter, warn};

const GRISWOLD_COMMIT: &str = env!("COMMIT_HASH");

pub mod input;
pub mod output;
pub mod preparation;
pub mod run;

/// Run benchmarks for the 2024 SOSP Paralegal paper.
///
/// The run is entirely guided by a [Config], which is read from the config-path
/// argument (defaults to "bench-config.toml").
///
/// Results are written to "result-path" (defaults to "results"). Each time you
/// call this program it creates a new set of directories. They all have the
/// format "<timestamp>-<purpose>" with the following purposes:
///
/// - "logs": stdout and stderr from the PDG generation (called "compile") and
///   combined output from the policy
/// - "pp": The source code that was involved in the analysis. The lines of code
///   actually visited.
/// - "run": Everything considered result data which are the following files:
///
///   - "results.csv": incrementally written statistics and results for each
///     run. Type [output::RunStat]
///   - "controllers.csv": incrementally written statistics about individual
///     controllers. Type [output::ControllerStat]. Multiple such statistics are
///     written for a single run. The "run_id" field tells you which run each
///     row belongs to.
///   - "sys.toml": information about the system that this experiment was run
///     on. Type [output::SysStat]
#[derive(clap::Parser)]
pub struct Arguments {
    /// Where to find the configuration file for this run
    config_path: PathBuf,
    /// Umbrella folder into which results should be written
    #[clap(long, default_value = "results")]
    result_path: PathBuf,
    #[clap(long)]
    no_install_flow_analyzer: bool,
    #[clap(short, long, conflicts_with_all = ["debug", "trace"])]
    verbose: bool,
    #[clap(long)]
    debug: bool,
    #[clap(long, conflicts_with = "debug")]
    trace: bool,
}

fn get_commit_version() -> String {
    std::process::Command::new("git")
        .args(["log", "-n", "1", "--format=%H"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or("unknown".to_owned())
        .trim()
        .to_owned()
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
    let args: &'static _ = Box::leak(Box::new(Arguments::parse()));
    let config_file = std::fs::read_to_string(&args.config_path)?;
    let config: EvaluationConfig = toml::from_str(&config_file)?;
    let rust_log_var = std::env::var("RUST_LOG");
    let verbosity = if args.trace {
        LevelFilter::TRACE
    } else if args.debug {
        LevelFilter::DEBUG
    } else if let Ok(lvl_str) = rust_log_var.as_ref() {
        lvl_str
            .parse()
            .map_err(anyhow::Error::from)
            .context("Parsing RUST_LOG env variable")?
    } else if let Some(lvl) = config.log_level {
        lvl
    } else {
        LevelFilter::WARN
    };

    env_setup();

    if rust_log_var.is_err() {
        std::env::set_var("RUST_LOG", "error");
    }

    // Force our toolchain for all compilations. We already force a specific
    // version of the stdlib in paralegal, but this also forces a specific
    // version for the tools. This is to work around the interface for the tools
    // changing, which we have observe with newer cargos passing "--check-cfg".
    std::env::set_var("RUSTUP_TOOLCHAIN", "nightly-2023-08-25");

    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(verbosity)
            .finish(),
    )?;

    let current_dir = std::env::current_dir()?;
    let paralegal_home_dir = &config.paralegal_home_dir;
    std::env::set_current_dir(paralegal_home_dir)?;
    if !args.no_install_flow_analyzer {
        info!(
            paralegal_home_dir = paralegal_home_dir.to_string_lossy().into_owned(),
            "Installing paralegal flow"
        );
        let compile_stat = process::Command::new("cargo")
            .args(["install", "--locked", "--path"])
            .arg(Path::new("crates").join("paralegal-flow"))
            .status()?;
        ensure!(compile_stat.success());
    }
    let paralegal_commit = get_commit_version();
    std::env::set_current_dir(current_dir)?;
    let this_commit_version = get_commit_version();

    if this_commit_version != GRISWOLD_COMMIT {
        warn!(GRISWOLD_COMMIT, this_commit_version, "WARN: This application was compiled from a different commit than the current state of the repo");
    }

    let mut output = Output::init(args, paralegal_commit, this_commit_version)?;

    config.run(&mut output)
}
