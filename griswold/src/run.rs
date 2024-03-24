//! Types that describe experiment runs and functions to execute them

use anyhow::Result;
use csv::Writer;
use indicatif::ProgressBar;
use paralegal_policy::Context;
use paralegal_policy::GraphLocation;
use std::fs::OpenOptions;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;
use std::{fs::File, path::PathBuf, sync::Arc, time::Instant, time::SystemTime};

use crate::input::Expectation;
use crate::Arguments;
use crate::{
    input::{ApplicationConfig, EvaluationConfig, ExperimentConfig},
    output::{CommandMeasurement, ControllerMeasurement, RunMeasurements, SystemParameters},
};

#[derive(Clone)]
pub struct Run<'c> {
    pub experiment_name: &'c str,
    pub config: &'c ExperimentConfig,
    pub app_config: &'c ApplicationConfig,
    pub policy_name: &'c str,
    pub comment: Option<&'c str>,
    pub expectation: Expectation,
    pub prepare: Option<Rc<dyn Fn()>>,
    pub policy: PolicyFn<'c>,
    pub extra_cargo_args: Vec<&'c str>,
}

impl Run<'_> {
    pub fn name(&self) -> String {
        let mut result = format!("{}-{}", self.config.application.as_ref(), self.policy_name);
        if let Some(comment) = self.comment {
            result.push('-');
            result.push_str(comment);
        }
        result
    }
}

pub type PolicyFn<'c> = Rc<dyn Fn(Arc<Context>) -> anyhow::Result<()> + 'c>;

pub struct Output {
    general_output_dir: PathBuf,
    pub controller_stat_out: Writer<File>,
    pub run_stat_out: Writer<File>,
}

impl Output {
    pub fn init(args: &Arguments) -> std::io::Result<Self> {
        let t = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let mut general_output_dir = args.result_path.clone();
        general_output_dir.push(format!("run-{t}"));
        assert!(!general_output_dir.exists());
        std::fs::create_dir_all(&general_output_dir)?;
        let sys_stat = SystemParameters::new();
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
            general_output_dir,
        })
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        self.controller_stat_out.flush()?;
        self.run_stat_out.flush()
    }

    pub fn path(&self, p: impl AsRef<Path>) -> PathBuf {
        self.general_output_dir.join(p)
    }
}

impl EvaluationConfig {
    pub fn run(&self, output: &mut Output) -> Result<()> {
        let experiments = self.experiments().enumerate().collect::<Vec<_>>();
        let progress = ProgressBar::new(experiments.len() as u64 * 2).with_style(
            indicatif::ProgressStyle::with_template(
                "[{msg:15}] {wide_bar} {pos:>4}/{len:4} {elapsed:7}",
            )?,
        );
        progress.enable_steady_tick(Duration::from_millis(500));
        let mut policy_out = File::create(output.path("policy.out.txt"))?;
        for (id, exp) in experiments {
            progress.inc(1);
            progress.set_message(format!("pdg: {}", exp.config.application.as_ref()));
            if let Some(prepare) = exp.prepare.as_ref() {
                (prepare)()
            }
            let compile_command = &mut exp.compile_cmd();
            let compile_dir = &exp.app_config.source_dir;
            progress.println(format!(
                "Running {:?} in {}",
                compile_command.get_command(),
                compile_dir.display(),
            ));
            let mut stdout = OpenOptions::new()
                .append(true)
                .create(true)
                .open(output.path("compile.stdout.txt"))?;
            let mut stderr = OpenOptions::new()
                .append(true)
                .create(true)
                .open(output.path("compile.stderr.txt"))?;
            use std::io::Write;
            writeln!(stdout, "{:?}", compile_command)?;
            writeln!(stderr, "{:?}", compile_command)?;
            let mut process = compile_command
                .get_command()
                .current_dir(&compile_dir)
                .stderr(stderr)
                .stdout(stdout)
                .spawn()?;
            let cmd_stat = CommandMeasurement::for_process(self, &mut process)?;
            let mut run_stats = RunMeasurements::from_experiment(id as u32, &exp, cmd_stat);
            progress.inc(1);
            progress.set_message(format!("policy: {}", exp.policy_name));
            if process.try_wait()?.unwrap().success() {
                let policy = exp.policy;
                let (res, cmd_stat) = CommandMeasurement::for_self(self, || {
                    let ctx = Arc::new(
                        GraphLocation::std(compile_dir)
                            .build_context(paralegal_policy::Config::default())?,
                    );
                    let policy_start = Instant::now();
                    (policy)(ctx.clone())?;
                    let success = ctx.emit_diagnostics(&mut policy_out)?;
                    anyhow::Ok((ctx, success, policy_start.elapsed()))
                });
                let (ctx, success, traversal_time) = res?;
                run_stats.add_policy_stat(cmd_stat, ctx.as_ref(), success, traversal_time);
                for ctrl in ctx.desc().controllers.values() {
                    output
                        .controller_stat_out
                        .serialize(ControllerMeasurement::from_spdg(id as u32, ctrl))?
                }
            } else {
                progress.println(format!(
                    "WARNING: Run id {} dir not successfully pass PDG construction",
                    id
                ));
            }
            output.run_stat_out.serialize(run_stats)?;
            output.flush()?;
        }
        Ok(())
    }
}
