//! Conversion from input types to run configurations / run preparation

use std::path::Path;
use std::process::{Command, Stdio};
use std::rc::Rc;

use clap::ValueEnum;
use lemmy::eval_driver::GetUserVersion;
use paralegal_policy::SPDGGenCommand;

use crate::input::{Application, EvaluationConfig, ExperimentConfig, ExperimentMode, PolicyResult};
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
                    self.ablation_runs_for_policy(
                        feature_space_success,
                        feature_space_fail,
                        name,
                        policy,
                    )
                },
            )),
            ExperimentMode::RollForward {
                start,
                end,
                expectation,
            } => {
                let commits = get_all_commits(
                    &self.evaluation_config.app_config[self.experiment_config.app_config_name()]
                        .source_dir,
                    start,
                    end,
                );
                Box::new(commits.into_iter().flat_map(move |c| {
                    self.experiment_config.application.policies().map(
                        move |(policy_name, policy)| {
                            let mut run = self.case_study_run(policy_name, policy, *expectation);
                            run.prepare = Some(Rc::new(checkout(&c)));
                            run.commit = Some(c.clone());
                            run
                        },
                    )
                }))
            }
        }
    }

    fn ablation_runs_for_policy(
        self,
        feature_space_success: &'a [String],
        feature_space_fail: &'a [String],
        policy_name: &'a str,
        policy: PolicyFn<'a>,
    ) -> impl Iterator<Item = Run<'a>> {
        let policy_clone = policy.clone();
        // An extra run to check that with no modifications this
        // policy version passes
        let canary_run = self.case_study_run(policy_name, policy.clone(), PolicyResult::Pass);
        let success_runs = feature_space_success.iter().map(move |feature| {
            let mut run = self.case_study_run(policy_name, policy.clone(), PolicyResult::Pass);
            run.extra_cargo_args.extend(["--features", &feature]);
            run.ablation_feature = Some(feature.as_str());
            run
        });
        let fail_runs = feature_space_fail.iter().map(move |feature| {
            let mut run =
                self.case_study_run(policy_name, policy_clone.clone(), PolicyResult::Fail);
            run.extra_cargo_args.extend(["--features", &feature]);
            run.ablation_feature = Some(feature.as_str());
            run
        });
        std::iter::once(canary_run)
            .chain(success_runs)
            .chain(fail_runs)
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
            .filter(|c| policies.contains(&c.property))
            .flat_map(move |batch_config| {
                let policy = |ctx| batch_config.property.run(ctx);
                let policy_name = batch_config.property.as_ref();
                let base_feature = batch_config.baseline_feature;
                macro_rules! mk_batch_exps {
                    ($expect_fail:expr, $controllers:expr, $extra_feature:expr) => {
                        $controllers.iter().map(move |c| {
                            let mut exp = self.case_study_run(
                                policy_name,
                                Rc::new(policy),
                                if $expect_fail {
                                    PolicyResult::Fail
                                } else {
                                    PolicyResult::Pass
                                },
                            );
                            exp.controller = Some(c);
                            exp.extra_cargo_args =
                                vec!["--features", c, "--features", base_feature];
                            if let Some(f) = $extra_feature {
                                exp.extra_cargo_args.extend(["--features", &f])
                            }
                            exp
                        })
                    };
                }
                let (initial_extra_feature, changed_extra_feature) =
                    if let Some(change) = batch_config.change.as_ref() {
                        if change.add_feature {
                            (None, Some(change.change_feature))
                        } else {
                            (Some(change.change_feature), None)
                        }
                    } else {
                        (None, None)
                    };
                batch_config
                    .baseline_controllers
                    .iter()
                    .flat_map(move |ctrl| {
                        mk_batch_exps!(batch_config.expect_failure, ctrl, initial_extra_feature)
                    })
                    .chain(batch_config.change.iter().flat_map(move |change| {
                        change.affected_controllers.into_iter().flat_map(move |c| {
                            mk_batch_exps!(batch_config.expect_failure, c, initial_extra_feature)
                        })
                    }))
                    .chain(batch_config.change.iter().flat_map(move |change| {
                        change.affected_controllers.iter().flat_map(move |c| {
                            mk_batch_exps!(!batch_config.expect_failure, c, changed_extra_feature)
                        })
                    }))
            })
    }

    fn case_study_run(
        self,
        policy_name: &'a str,
        policy: PolicyFn<'a>,
        expectation: PolicyResult,
    ) -> Run<'a> {
        let mut run = Run::new(
            self.experiment_name,
            self.experiment_config,
            self.evaluation_config,
            policy_name,
            policy,
            expectation,
        );
        if let Application::Websubmit { flavour, .. } = &self.experiment_config.application {
            run.extra_cargo_args
                .extend(["--features", flavour.annotation_feature()]);
        }
        run
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

fn get_all_commits(path: impl AsRef<Path>, start: &str, end: &str) -> Vec<String> {
    let output = Command::new("git")
        .args(["log", &format!("{start}..{end}"), "--format=%H"])
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
    fn expectations(&self) -> &'static [(PolicyResult, &'static [&'static str])] {
        match self {
            Application::AtomicData => &[
                (PolicyResult::Pass, &["--features", "bug-fix"]),
                (PolicyResult::Fail, &[]),
            ],
            Application::Lemmy { .. } => unimplemented!("Lemmy requires special handling"),
            Application::Freedit { .. } => &[
                (PolicyResult::Pass, &[]),
                (PolicyResult::Fail, &["--features", "buggy"]),
            ],
            Application::Hyperswitch { .. } => &[(PolicyResult::Pass, &[])],
            Application::Plume => &[
                (PolicyResult::Fail, &[]),
                (
                    PolicyResult::Pass,
                    &["--features", "plume-models/delete-comments"],
                ),
            ],
            Application::Websubmit { .. } => &[(PolicyResult::Pass, &[])],
        }
    }

    fn policies<'a>(&'a self) -> impl Iterator<Item = (&'a str, PolicyFn<'a>)> {
        match self {
            Application::AtomicData => Box::new(std::iter::once((
                "check-writes",
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
                "data-deletion",
                Rc::new(plume::check) as PolicyFn<'a>,
            ))),
            Application::Websubmit { policies, flavour } => Box::new(
                selection_or_all(policies)
                    .iter()
                    .map(|p| (p.as_ref(), Rc::from(p.runnable(*flavour)) as PolicyFn<'a>)),
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
