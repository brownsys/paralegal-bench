//! Conversion from input types to run configurations / run preparation

use std::path::Path;
use std::process::{Command, Stdio};
use std::rc::Rc;

use clap::ValueEnum;
use lemmy::eval_driver::GetUserVersion;
use paralegal_policy::SPDGGenCommand;

use crate::input::{Application, EvaluationConfig, Expectation, ExperimentConfig, ExperimentMode};
use crate::run::{PolicyFn, Run};

fn selection_or_all<V: ValueEnum>(policies: &[V]) -> &[V] {
    if policies.is_empty() {
        V::value_variants()
    } else {
        policies
    }
}

impl EvaluationConfig {
    pub fn experiments(&self) -> impl Iterator<Item = Run<'_>> {
        self.experiment
            .iter()
            .flat_map(move |(experiment_name, es)| {
                es.iter().flat_map(move |experiment_config| {
                    RunBuilder {
                        experiment_config,
                        experiment_name,
                        evaluation_config: self,
                    }
                    .into_experiments()
                })
            })
    }
}

impl ExperimentConfig {
    pub fn app_config_name(&self) -> &str {
        if let Some(name) = self.app_config_override.as_ref() {
            name.as_str()
        } else {
            self.application.as_ref()
        }
    }
}

#[derive(Clone, Copy)]
struct RunBuilder<'a> {
    experiment_name: &'a str,
    experiment_config: &'a ExperimentConfig,
    evaluation_config: &'a EvaluationConfig,
}

impl<'a> RunBuilder<'a> {
    pub fn into_experiments(self) -> impl Iterator<Item = Run<'a>> + 'a {
        match &self.experiment_config.mode {
            ExperimentMode::CaseStudy => {
                Box::new(self.case_study_runs()) as Box<dyn Iterator<Item = Run<'a>> + 'a>
            }
            ExperimentMode::Ablation {
                feature_space_success,
                feature_space_fail,
            } => Box::new(self.experiment_config.application.policies().flat_map(
                move |(name, policy)| {
                    let policy_clone = policy.clone();
                    feature_space_success
                        .iter()
                        .map(move |feature| {
                            let mut run =
                                self.case_study_run(name, policy.clone(), Expectation::Pass);
                            run.extra_cargo_args.extend(["--features", &feature]);
                            run
                        })
                        .chain(feature_space_fail.iter().map(move |feature| {
                            let mut run =
                                self.case_study_run(name, policy_clone.clone(), Expectation::Fail);
                            run.extra_cargo_args.extend(["--features", &feature]);
                            run
                        }))
                },
            )),
            ExperimentMode::RollForward {
                pass_threshold,
                fail_threshold,
                starting_expectation,
                limit,
                start_commit: starting_commit,
            } => {
                let mut expectation = *starting_expectation;
                let mut commits = get_all_commits(
                    &self.evaluation_config.app_config[self.experiment_config.app_config_name()]
                        .source_dir,
                    &starting_commit,
                );
                if let Some(limit) = limit {
                    commits.truncate(*limit);
                }
                Box::new(commits.into_iter().flat_map(move |c| {
                    let current_expectation = expectation;
                    if fail_threshold.contains(&c) {
                        expectation = Expectation::Pass;
                    }
                    if pass_threshold.contains(&c) {
                        expectation = Expectation::Fail;
                    }
                    self.experiment_config.application.policies().map(
                        move |(policy_name, policy)| {
                            let mut run =
                                self.case_study_run(policy_name, policy, current_expectation);
                            run.prepare = Some(Rc::new(checkout(&c)));
                            run
                        },
                    )
                }))
            }
        }
    }

    fn case_study_runs(self) -> impl Iterator<Item = Run<'a>> {
        match &self.experiment_config.application {
            Application::Lemmy { policies } => {
                Box::new(self.lemmy_case_study(selection_or_all(policies)))
                    as Box<dyn Iterator<Item = _> + 'a>
            }
            _ => Box::new(
                self.experiment_config
                    .application
                    .expectations()
                    .iter()
                    .copied()
                    .flat_map(move |(expectation, cargo_args)| {
                        self.experiment_config.application.policies().map(
                            move |(name, policy_fn)| {
                                let mut run = self.case_study_run(name, policy_fn, expectation);
                                run.extra_cargo_args.extend(cargo_args);
                                run
                            },
                        )
                    }),
            ),
        }
    }

    fn lemmy_case_study(self, policies: &'a [lemmy::Prop]) -> impl Iterator<Item = Run<'a>> {
        GetUserVersion::value_variants()
            .iter()
            .map(|v| v.to_config())
            .filter(|c| policies.contains(&c.property.into()))
            .flat_map(move |batch_config| {
                let policy = |ctx| batch_config.property.run(ctx);
                let policy_name = batch_config.property.as_ref();
                macro_rules! mk_batch_exps {
                    ($expect_fail:expr, $controllers:expr) => {
                        $controllers.iter().map(move |c| {
                            let mut exp = self.case_study_run(
                                policy_name,
                                Rc::new(policy),
                                if $expect_fail {
                                    Expectation::Fail
                                } else {
                                    Expectation::Pass
                                },
                            );
                            exp.comment = Some(c);
                            exp.extra_cargo_args = vec!["--features", c];
                            exp
                        })
                    };
                }
                batch_config
                    .baseline_controllers
                    .iter()
                    .flat_map(move |ctrl| mk_batch_exps!(batch_config.expect_failure, ctrl))
                    .chain(batch_config.change.iter().flat_map(move |change| {
                        change
                            .affected_controllers
                            .into_iter()
                            .flat_map(move |c| mk_batch_exps!(batch_config.expect_failure, c))
                    }))
                    .chain(batch_config.change.iter().flat_map(move |change| {
                        change
                            .affected_controllers
                            .iter()
                            .flat_map(move |c| mk_batch_exps!(!batch_config.expect_failure, c))
                    }))
            })
    }

    fn case_study_run(
        self,
        policy_name: &'a str,
        policy: PolicyFn<'a>,
        expectation: Expectation,
    ) -> Run<'a> {
        let app_config =
            &self.evaluation_config.app_config[self.experiment_config.app_config_name()];
        Run {
            experiment_name: self.experiment_name,
            config: self.experiment_config,
            policy_name,
            app_config,
            policy,
            expectation,
            prepare: None,
            comment: None,
            extra_cargo_args: vec![],
        }
    }
}

fn checkout(s: &str) -> impl Fn(Stdio, Stdio) {
    let s = s.to_owned();
    move |stdout, stderr| {
        assert!(Command::new("git")
            .args(["checkout", "--force", &s])
            .stdout(stdout)
            .stderr(stderr)
            .status()
            .unwrap()
            .success())
    }
}

fn get_all_commits(path: impl AsRef<Path>, start: &str) -> Vec<String> {
    let output = Command::new("git")
        .args(["log", "--format=%H", start])
        .current_dir(path)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

impl Application {
    /// Default expectations for each application used in the "case-study"
    /// experiment mode.
    fn expectations(&self) -> &'static [(Expectation, &'static [&'static str])] {
        match self {
            Application::AtomicData => &[
                (Expectation::Pass, &["--features", "bug-fix"]),
                (Expectation::Fail, &[]),
            ],
            Application::Lemmy { .. } => unimplemented!("Lemmy requires special handling"),
            Application::Freedit { .. } => &[
                (Expectation::Pass, &[]),
                (Expectation::Fail, &["--features", "buggy"]),
            ],
            Application::Hyperswitch { .. } => &[(Expectation::Pass, &[])],
            Application::Plume => &[
                (Expectation::Fail, &[]),
                (
                    Expectation::Pass,
                    &["--features", "plume-models/delete-comments"],
                ),
            ],
            Application::Websubmit { .. } => &[(Expectation::Pass, &[])],
        }
    }

    fn policies<'a>(&'a self) -> impl Iterator<Item = (&'a str, PolicyFn<'a>)> {
        match self {
            Application::AtomicData => Box::new(std::iter::once((
                "atomic",
                Rc::new(atomic::check_rights) as PolicyFn<'a>,
            )))
                as Box<dyn Iterator<Item = (&'a str, PolicyFn<'a>)>>,
            Application::Freedit { policies } => Box::new(
                selection_or_all(policies)
                    .iter()
                    .map(|p| (p.as_ref(), Rc::new(move |ctx| p.check(ctx)) as PolicyFn<'a>)),
            ),
            Application::Hyperswitch { policies } => Box::new(
                selection_or_all(policies)
                    .iter()
                    .map(|p| (p.as_ref(), Rc::new(p.runnable()) as PolicyFn<'a>)),
            ),
            Application::Lemmy { policies } => Box::new(
                selection_or_all(policies)
                    .iter()
                    .map(|p| (p.as_ref(), Rc::new(move |cx| p.run(cx)) as PolicyFn<'a>)),
            ),
            Application::Plume => Box::new(std::iter::once((
                "plume",
                Rc::new(plume::check) as PolicyFn<'a>,
            ))),
            Application::Websubmit { policies } => Box::new(
                selection_or_all(policies)
                    .iter()
                    .map(|p| (p.as_ref(), Rc::new(p.runnable()) as PolicyFn<'a>)),
            ),
        }
    }
}

impl Run<'_> {
    pub fn compile_cmd(&self) -> SPDGGenCommand {
        let app_config = self.app_config;
        let mut compile_cmd = SPDGGenCommand::global();
        if let Some(path) = app_config.external_annotations.as_ref() {
            compile_cmd.external_annotations(path);
        }
        if app_config.abort {
            compile_cmd.abort_after_analysis();
        }
        if self.config.adaptive_depth {
            compile_cmd.get_command().arg("--adaptive-depth");
        }
        compile_cmd
            .get_command()
            .args(app_config.flow_args.iter())
            .arg("--")
            .args(app_config.cargo_args.iter())
            .args(self.extra_cargo_args.iter());
        compile_cmd
    }
}
