use clap::Parser;
use input::Config;
use run::Output;
use std::path::PathBuf;

pub mod conversion;
pub mod input;
pub mod output;
pub mod run;

#[derive(clap::Parser)]
pub struct Arguments {
    #[clap(default_value = "bench-config.toml")]
    config_path: PathBuf,
    #[clap(default_value = "results")]
    result_path: PathBuf,
}

fn main() {
    let args: &'static _ = Box::leak(Box::new(Arguments::parse()));
    let mut output = Output::init(args).unwrap();
    let config_file = std::fs::read_to_string(&args.config_path).unwrap();
    let config: Config = toml::from_str(&config_file).unwrap();

    config.run(&mut output).unwrap()
}
