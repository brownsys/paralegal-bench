//! Types that describe experiment runs and functions to execute them

use anyhow::{ensure, Result};
use cargo::{
    core::Workspace,
    ops::{resolve_ws, UpdateOptions},
    util::important_paths::find_root_manifest_for_wd,
};
use chrono;
use csv::Writer;
use indicatif::ProgressBar;
use paralegal_policy::{Context, GraphLocation};
use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
use tracing::{info, trace, warn};

use crate::{
    input::{ApplicationConfig, CrateOverride, EvaluationConfig, ExperimentConfig, PolicyResult},
    output::{CommandMeasurement, ControllerMeasurement, RunMeasurements, SystemParameters},
    Arguments,
};

#[derive(Clone)]
pub struct Run<'c> {
    pub experiment_name: &'c str,
    pub config: &'c ExperimentConfig,
    pub app_config: &'c ApplicationConfig,
    pub policy_name: &'c str,
    pub external_annotations: Option<&'c Path>,
    /// Only set when a single controller is selected (used in Lemmy)
    pub controller: Option<&'c str>,
    /// Only set in ablation experiments. This feature is what selects the
    /// ablation configuration
    pub ablation_feature: Option<&'c str>,
    /// Only set in roll-forward experiments. Denotes the commit this run is
    /// performed on.
    pub commit: Option<String>,
    pub bug: Option<&'c str>,
    pub expectation: PolicyResult,
    /// Called before the analyzer runs. Arguments are a handle to use as stdout
    /// and stderr
    pub prepare: Option<Rc<dyn Fn(Stdio, Stdio)>>,
    pub post_process: Option<Rc<dyn Fn(&Context, &mut RunMeasurements)>>,
    pub policy: PolicyFn<'c>,
    pub extra_cargo_args: Vec<&'c str>,
}

impl<'a> Run<'a> {
    pub fn name(&self) -> String {
        let mut result = format!("{}-{}", self.config.application.as_ref(), self.policy_name);
        if let Some(comment) = self.ablation_feature.as_ref() {
            result.push('-');
            result.push_str(comment.as_ref());
        }
        result
    }

    pub fn new(
        experiment_name: &'a str,
        experiment_config: &'a ExperimentConfig,
        evaluation_config: &'a EvaluationConfig,
        policy_name: &'a str,
        policy: PolicyFn<'a>,
        expectation: PolicyResult,
    ) -> Run<'a> {
        let app_config = &evaluation_config.app_config[experiment_config.app_config_name()];
        Self {
            experiment_name,
            config: experiment_config,
            external_annotations: None,
            policy_name,
            app_config,
            policy,
            expectation,
            prepare: None,
            bug: None,
            ablation_feature: None,
            commit: None,
            controller: None,
            extra_cargo_args: vec![],
            post_process: None,
        }
    }
}

pub type PolicyFn<'c> = Rc<dyn Fn(Arc<Context>) -> anyhow::Result<()> + 'c>;

pub struct Output {
    general_output_dir: PathBuf,
    post_process_dir: PathBuf,
    pub controller_stat_out: Writer<File>,
    pub run_stat_out: Writer<File>,
}

impl Output {
    pub fn init(
        args: &Arguments,
        paralegal_commit: String,
        repo_commit: String,
    ) -> std::io::Result<Self> {
        let bench_num = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S");
        std::fs::create_dir_all(&args.result_path)?;
        let mut general_output_dir = args.result_path.canonicalize()?;
        let post_process_dir = general_output_dir.join(format!("{bench_num}-pp"));
        let metrics_output_dir = general_output_dir.join(format!("{bench_num}-run"));
        general_output_dir.push(format!("{bench_num}-logs"));
        for dir in [&general_output_dir, &post_process_dir, &metrics_output_dir] {
            assert!(!dir.exists(), "{}", dir.display());
            std::fs::create_dir(dir)?;
        }
        std::fs::copy(
            &args.config_path,
            metrics_output_dir.join("bench-config.toml"),
        )?;
        let sys_stat = SystemParameters::new(paralegal_commit, repo_commit);
        let mut sys_stat_file = File::create(metrics_output_dir.join("sys.toml"))?;
        use std::io::Write;
        write!(
            sys_stat_file,
            "{}",
            toml::to_string_pretty(&sys_stat).unwrap()
        )
        .unwrap();
        Ok(Self {
            controller_stat_out: Writer::from_path(metrics_output_dir.join("controllers.csv"))?,
            run_stat_out: Writer::from_path(metrics_output_dir.join("results.csv"))?,
            general_output_dir,
            post_process_dir,
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

impl Output {
    fn log_for(&self, prefix: &str) -> std::io::Result<(File, File)> {
        let stdout = OpenOptions::new()
            .append(true)
            .create(true)
            .open(self.path(format!("{prefix}.stdout.txt")))?;
        let stderr = OpenOptions::new()
            .append(true)
            .create(true)
            .open(self.path(format!("{prefix}.stderr.txt")))?;
        Ok((stdout, stderr))
    }
}

impl CrateOverride {
    /// Assumes we are in the directory of the crate we want to make changes to
    fn enact(&self, name: &str, stdout: Box<dyn std::io::Write>) -> Result<()> {
        let cargo_cfg = cargo::Config::default()?;
        let mut shell = cargo::core::Shell::from_write(stdout);
        shell.set_verbosity(cargo::core::Verbosity::Quiet);
        *cargo_cfg.shell() = shell;
        //cargo_cfg.configure(0, true, None, false, false, false, &None, &[], &[])?;
        let current_dir = std::env::current_dir()?;
        let ws = Workspace::new(&find_root_manifest_for_wd(&current_dir)?, &cargo_cfg)?;
        // Might have to change this, use a specific config and enable/disable certain features
        let (_package_set, graph) = resolve_ws(&ws)?;
        let interned_name: cargo::util::interning::InternedString = name.into();
        for p in graph.iter() {
            let summary = graph.summary(p);
            if summary.name() == interned_name && self.original.matches(summary.version()) {
                cargo::ops::update_lockfile(
                    &ws,
                    &UpdateOptions {
                        config: &cargo_cfg,
                        to_update: vec![format!("{name}@{}", summary.version())],
                        precise: Some(&self.replacement.to_string()),
                        recursive: false,
                        dry_run: false,
                        workspace: false,
                    },
                )?
            }
        }
        Ok(())
    }
}

fn with_env<'a, R>(
    overrides: impl IntoIterator<Item = (&'a str, &'a str)>,
    f: impl FnOnce() -> R,
) -> R {
    let to_restore = overrides
        .into_iter()
        .map(|(k, v)| {
            let old = std::env::var_os(k);
            std::env::set_var(k, v);
            (k, old)
        })
        .collect::<Vec<_>>();
    let r = f();
    for (k, v) in to_restore {
        if let Some(old) = v {
            std::env::set_var(k, old);
        } else {
            std::env::remove_var(k);
        }
    }
    r
}

impl EvaluationConfig {
    pub fn run(&self, output: &mut Output) -> Result<()> {
        for (app_name, app_config) in self.app_config.iter() {
            trace!(app_name, "Checking app for whether folder exists");
            let app_src = &app_config.source_dir;
            if !app_src.exists() {
                if let Some(repo) = app_config.clone.as_ref() {
                    info!(
                        app_name,
                        repo,
                        app_dir = app_src.to_string_lossy().into_owned(),
                        "Cloning non existent app"
                    );
                    let success = Command::new("git")
                        .args(["clone", repo])
                        .arg(app_src.as_os_str())
                        .status()?;
                    ensure!(
                        success.success(),
                        "Could not clone {app_name} ({repo}) to {}",
                        app_src.display()
                    );
                } else {
                    warn!(
                        app_name,
                        app_dir = app_src.to_string_lossy().into_owned(),
                        "Directory for application does not exist. The run may fail"
                    );
                }
            }
        }
        let post_process_dir = &output.post_process_dir;
        let experiments = self
            .experiments(&post_process_dir)
            .enumerate()
            .collect::<Vec<_>>();
        let progress = ProgressBar::new(experiments.len() as u64 * 2).with_style(
            indicatif::ProgressStyle::with_template(
                "[{msg:15}] {wide_bar} {pos:>4}/{len:4} {elapsed:7}",
            )?,
        );
        progress.enable_steady_tick(Duration::from_millis(500));
        let mut policy_out = Arc::new(File::create(output.path("policy.out.txt"))?);
        let starting_dir = std::env::current_dir()?;
        for (id, exp) in experiments.iter() {
            let Run {
                experiment_name,
                config,
                policy_name,
                controller,
                ablation_feature,
                commit,
                ..
            } = exp;
            trace!(
                "Running {id} {experiment_name} {} {} {policy_name} {controller:?} {ablation_feature:?} {commit:?}", config.application.as_ref(), config.controller_run_mode.as_ref(),
            );
        }
        for (id, exp) in experiments {
            std::env::set_current_dir(&exp.app_config.source_dir)?;
            with_env(
                exp.app_config
                    .env
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str())),
                || {
                    progress.inc(1);
                    progress.set_message(format!("pdg: {}", exp.config.application.as_ref()));
                    if let Some(prepare) = exp.prepare.as_ref() {
                        let (stdout, stderr) = output.log_for("prepare")?;
                        (prepare)(stdout.into(), stderr.into())
                    }
                    for (package, overrides) in &exp.app_config.version_override {
                        let (stdout, _stderr) = output.log_for("prepare")?;
                        overrides.enact(package, Box::new(stdout))?;
                    }
                    let compile_command = &mut exp.compile_cmd();
                    //progress.println(format!("Running {} {:?}", compile_command.get_command(),));
                    let (mut stdout, mut stderr) = output.log_for("compile")?;
                    use std::io::Write;
                    writeln!(stdout, "###### Run {id}: {:?}", compile_command)?;
                    writeln!(stderr, "###### Run {id}: {:?}", compile_command)?;
                    let mut process = compile_command
                        .get_command()
                        .stderr(stderr)
                        .stdout(stdout)
                        .spawn()?;
                    let cmd_stat =
                        CommandMeasurement::for_process(self, self.pdg_timeout, &mut process)?;
                    let mut run_stats = RunMeasurements::from_experiment(id as u32, &exp, cmd_stat);
                    progress.inc(1);
                    progress.set_message(format!("policy: {}", exp.policy_name));
                    match process.try_wait()? {
                        Some(e) if e.success() => {
                            let policy = exp.policy;
                            let (res, cmd_stat) = CommandMeasurement::for_self(self, || {
                                let graph_loc = GraphLocation::std(".");
                                let file_size = graph_loc.path().metadata().map_or(0, |d| d.len());
                                let mut config = paralegal_policy::Config::default();
                                //config.output_writer = Box::new(policy_out.clone());
                                let ctx = Arc::new(graph_loc.build_context(config)?);
                                let policy_start = Instant::now();
                                (policy)(ctx.clone())?;
                                writeln!(policy_out, "###### Run {id}: {:?}", compile_command)?;
                                let success = ctx.emit_diagnostics(policy_out.clone())?;
                                anyhow::Ok((ctx, success, file_size, policy_start.elapsed()))
                            });
                            let (ctx, success, file_size, traversal_time) = res?;
                            run_stats.add_policy_stat(
                                cmd_stat,
                                ctx.as_ref(),
                                if success {
                                    PolicyResult::Pass
                                } else {
                                    PolicyResult::Fail
                                },
                                traversal_time,
                                file_size,
                            );
                            for ctrl in ctx.desc().controllers.values() {
                                output
                                    .controller_stat_out
                                    .serialize(ControllerMeasurement::from_spdg(id as u32, ctrl))?
                            }
                            if let Some(pp) = exp.post_process.as_ref() {
                                pp(&ctx, &mut run_stats);
                            }
                        }
                        other => {
                            progress.println(format!(
                        "WARNING: Run id {} dir not successfully pass PDG construction: {other:?}",
                        id
                    ));
                        }
                    }
                    output.run_stat_out.serialize(run_stats)?;
                    output.flush()?;
                    anyhow::Ok(())
                },
            )?;
            std::env::set_current_dir(&starting_dir)?;
        }
        Ok(())
    }
}
