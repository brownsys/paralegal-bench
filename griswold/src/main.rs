use clap::Parser;
use input::EvaluationConfig;
use run::Output;
use std::{
    path::{Path, PathBuf},
    process,
};

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
/// call this program it creates a new directory called "run-<current time in
/// seconds>".
///
/// Into this directory it creates the following files:
///
/// - "results.csv": incrementally written statistics and results for each run.
///   Type [output::RunStat]
/// - "controllers.csv": incrementally written statistics about individual
///   controllers. Type [output::ControllerStat]. Multiple such statistics are
///   written for a single run. The "run_id" field tells you which run each row
///   belongs to.
/// - "sys.toml": information about the system that this experiment was run on.
///   Type [output::SysStat]
#[derive(clap::Parser)]
pub struct Arguments {
    /// Where to find the configuration file for this run
    #[clap(long, default_value = "bench-config.toml")]
    config_path: PathBuf,
    /// Umbrella folder into which results should be written
    #[clap(long, default_value = "results")]
    result_path: PathBuf,
    #[clap(long)]
    no_install_flow_analyzer: bool,
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

fn main() {
    let args: &'static _ = Box::leak(Box::new(Arguments::parse()));
    let config_file = std::fs::read_to_string(&args.config_path).unwrap();
    let config: EvaluationConfig = toml::from_str(&config_file).unwrap();

    let current_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&config.paralegal_home_dir).unwrap();
    if !args.no_install_flow_analyzer {
        let compile_stat = process::Command::new("cargo")
            .args(["install", "--locked", "--path"])
            .arg(Path::new("crates").join("paralegal-flow"))
            .status()
            .unwrap();
        assert!(compile_stat.success());
    }
    let paralegal_commit = get_commit_version();
    std::env::set_current_dir(current_dir).unwrap();
    let this_commit_version = get_commit_version();

    if this_commit_version != GRISWOLD_COMMIT {
        println!("WARN: This application was compiled from a different commit ({GRISWOLD_COMMIT}) than the current state of the repo ({this_commit_version})");
    }

    let mut output = Output::init(args, paralegal_commit, this_commit_version).unwrap();

    config.run(&mut output).unwrap()
}
