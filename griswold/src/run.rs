//! Types that describe experiment runs and functions to execute them

use csv::Writer;
use paralegal_policy::{Context, SPDGGenCommand};
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use crate::input::{ApplicationConfig, ExperimentConfig};
use crate::output::SysStat;

pub struct Experiment<'c> {
    pub config: &'c ExperimentConfig,
    pub app_config: &'c ApplicationConfig,
    pub policy_name: &'c str,
    pub comment: Option<&'c str>,
    pub expectation: bool,
    pub prepare: Option<Box<dyn Fn()>>,
    pub policy: PolicyFn<'c>,
    pub compile_cmd: SPDGGenCommand,
}

impl Experiment<'_> {
    pub fn name(&self) -> String {
        let mut result = format!("{}-{}", self.config.application.as_ref(), self.policy_name);
        if let Some(comment) = self.comment {
            result.push('-');
            result.push_str(comment);
        }
        result
    }
}

pub type PolicyFn<'c> = Box<dyn Fn(Arc<Context>) -> anyhow::Result<()> + 'c>;

pub struct Output {
    pub controller_stat_out: Writer<File>,
    pub run_stat_out: Writer<File>,
}

impl Output {
    pub fn init() -> std::io::Result<Self> {
        let t = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let general_output_dir: PathBuf = format!("run-{t}").into();
        let sys_stat = SysStat::new();
        let mut sys_stat_file = File::create(general_output_dir.join("sys.toml"))?;
        use std::io::Write;
        write!(
            sys_stat_file,
            "{}",
            toml::to_string_pretty(&sys_stat).unwrap()
        )
        .unwrap();
        Ok(Self {
            controller_stat_out: Writer::from_path(general_output_dir.join("controllers.csv"))?,
            run_stat_out: Writer::from_path(general_output_dir.join("results.csv"))?,
        })
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        self.controller_stat_out.flush()?;
        self.run_stat_out.flush()
    }
}
