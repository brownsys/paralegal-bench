//! Conversion from input types to run configurations / run preparation

use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::Arc;

use clap::ValueEnum;
use paralegal_policy::{Context, SPDGGenCommand};

use crate::input::{
    Application, ControllerRunMode, EvaluationConfig, ExperimentConfig, ExperimentMode, PolicyMode,
    PolicyResult, RollForwardCutoff,
};
use crate::output::RunMeasurements;
use crate::run::{PolicyFn, Run};

mod lemmy;

fn selection_or_all<V: ValueEnum>(policies: &[V]) -> &[V] {
    if policies.is_empty() {
        V::value_variants()
    } else {
        policies
    }
}

impl EvaluationConfig {
    pub fn experiments<'a, 'b: 'a>(
        &'b self,
        target_path: &'a Path,
    ) -> impl Iterator<Item = Run<'b>> + 'a {
        self.experiment
            .iter()
            .flat_map(move |(experiment_name, es)| {
                es.iter().flat_map(move |experiment_config| {
                    RunBuilder {
                        experiment_config,
                        experiment_name,
                        evaluation_config: self,
                    }
                    .into_experiments(target_path)
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
    pub fn policies(self) -> impl Iterator<Item = (&'a str, PolicyFn<'a>, Vec<&'a str>)> {
        match self.experiment_config.policy_mode {
            PolicyMode::Separate => Box::new(self.experiment_config.application.policies())
                as Box<dyn Iterator<Item = _> + 'a>,
            PolicyMode::Unified => Box::new(std::iter::once(self.unified_policies())),
        }
    }

    pub fn unified_policies(self) -> (&'a str, PolicyFn<'a>, Vec<&'a str>) {
        let base = self
            .experiment_config
            .application
            .policies()
            .collect::<Vec<_>>();
        let affected = base
            .iter()
            .flat_map(|(_, _, p)| p.iter())
            .copied()
            .collect();
        (
            "unified",
            Rc::new(move |ctx: Arc<Context>| {
                for (_, policy, _) in base.iter() {
                    policy(ctx.clone())?;
                }
                Ok(())
            }) as _,
            affected,
        )
    }

    pub fn into_experiments<'b>(self, target_path: &'b Path) -> impl Iterator<Item = Run<'a>> + 'b
    where
        'a: 'b,
    {
        match &self.experiment_config.mode {
            ExperimentMode::CaseStudy => {
                Box::new(self.case_study_runs()) as Box<dyn Iterator<Item = Run<'a>> + 'a>
            }
            ExperimentMode::Ablation {
                feature_space_success,
                feature_space_fail,
            } => Box::new(self.policies().flat_map(move |(name, policy, affected)| {
                self.ablation_runs_for_policy(
                    feature_space_success,
                    feature_space_fail,
                    name,
                    policy,
                    affected,
                )
            })),
            ExperimentMode::RollForward { cutoff } => {
                Box::new(self.roll_forward_runs(target_path, cutoff))
            }
        }
    }

    fn roll_forward_runs<'b>(
        self,
        target_path: &'b Path,
        cutoff: &'a [RollForwardCutoff],
    ) -> impl Iterator<Item = Run<'a>> + 'b
    where
        'a: 'b,
    {
        let app_dir =
            &self.evaluation_config.app_config[self.experiment_config.app_config_name()].source_dir;

        let (commit_range, conf): (Vec<_>, Vec<_>) = (0..cutoff.len())
            .rev()
            .flat_map(|cidx| {
                let current = &cutoff[cidx];
                current
                    .expectation
                    .map(|expectation| {
                        let next_commit = (cidx != 0)
                            .then(|| cutoff.get(cidx - 1))
                            .flatten()
                            .map(|c| &c.commit);
                        if let Some(next) = next_commit.as_ref() {
                            get_all_commits(app_dir, &next, &current.commit)
                        } else {
                            vec![current.commit.clone()]
                        }
                        .into_iter()
                        .zip(std::iter::repeat((current, expectation)))
                    })
                    .into_iter()
                    .flatten()
            })
            .unzip();
        let commit_range = Rc::new(commit_range);

        commit_range
            .clone()
            .iter()
            .zip(conf.iter())
            .enumerate()
            .flat_map(move |(idx, (commit, (current, expectation)))| {
                let commit_range = commit_range.clone();
                self.policies()
                    .flat_map(move |(policy_name, policy, affected)| {
                        let commit_range = commit_range.clone();
                        self.controllers(affected).map(move |c| {
                            let mut run =
                                self.case_study_run(policy_name, policy.clone(), *expectation, c);
                            run.external_annotations =
                                current.external_annotations.as_ref().map(|pb| pb.as_path());
                            run.prepare = Some(Rc::new(checkout(&commit)));
                            run.post_process = Some(Rc::new(diff_analyzed(
                                idx,
                                commit_range.clone(),
                                target_path,
                            )));
                            run.commit = Some(commit.clone());
                            run
                        })
                    })
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn ablation_runs_for_policy(
        self,
        feature_space_success: &'a [String],
        feature_space_fail: &'a [String],
        policy_name: &'a str,
        policy: PolicyFn<'a>,
        affected: Vec<&'a str>,
    ) -> impl Iterator<Item = Run<'a>> {
        self.controllers(affected).flat_map(move |c| {
            let policy_clone = policy.clone();
            let policy_clone_2 = policy.clone();
            let c_clone_1 = c.clone();
            let c_clone_2 = c.clone();
            // An extra run to check that with no modifications this
            // policy version passes
            let canary_run =
                self.case_study_run(policy_name, policy.clone(), PolicyResult::Pass, c);
            let success_runs = feature_space_success.iter().map(move |feature| {
                let mut run = self.case_study_run(
                    policy_name,
                    policy_clone_2.clone(),
                    PolicyResult::Pass,
                    c_clone_1.clone(),
                );
                run.extra_cargo_args.extend(["--features", &feature]);
                run.ablation_feature = Some(feature.as_str());
                run
            });
            let fail_runs = feature_space_fail.iter().map(move |feature| {
                let mut run = self.case_study_run(
                    policy_name,
                    policy_clone.clone(),
                    PolicyResult::Fail,
                    c_clone_2.clone(),
                );
                run.extra_cargo_args.extend(["--features", &feature]);
                run.ablation_feature = Some(feature.as_str());
                run
            });
            std::iter::once(canary_run)
                .chain(success_runs)
                .chain(fail_runs)
        })
    }

    fn controllers(self, affected: Vec<&'a str>) -> impl Iterator<Item = Vec<&'a str>> {
        match self.experiment_config.controller_run_mode {
            ControllerRunMode::Affected => {
                Box::new(std::iter::once(affected)) as Box<dyn Iterator<Item = _>>
            }
            // Special case if no controllers are defined -> run anyway
            ControllerRunMode::AffectedMerged if affected.is_empty() => {
                Box::new(std::iter::once(affected))
            }
            ControllerRunMode::AffectedMerged => {
                Box::new(affected.into_iter().map(|c| vec![c])) as Box<_>
            }
            ControllerRunMode::All => Box::new(std::iter::once(
                self.experiment_config
                    .application
                    .all_controllers()
                    .to_vec(),
            )) as Box<_>,
        }
    }

    fn case_study_runs(self) -> impl Iterator<Item = Run<'a>> {
        match &self.experiment_config.application {
            Application::Lemmy { policies, bugs } => {
                Box::new(self.lemmy_case_study(selection_or_all(policies), bugs))
                    as Box<dyn Iterator<Item = _> + 'a>
            }
            _ => Box::new(
                self.experiment_config
                    .application
                    .expectations()
                    .iter()
                    .copied()
                    .flat_map(move |(expectation, cargo_args)| {
                        self.policies()
                            .flat_map(move |(name, policy_fn, affected)| {
                                self.controllers(affected).map(move |c| {
                                    let mut run = self.case_study_run(
                                        name,
                                        policy_fn.clone(),
                                        expectation,
                                        c,
                                    );
                                    run.extra_cargo_args.extend(cargo_args);
                                    run
                                })
                            })
                    }),
            ),
        }
    }

    fn case_study_run(
        self,
        policy_name: &'a str,
        policy: PolicyFn<'a>,
        expectation: PolicyResult,
        controllers: impl IntoIterator<Item = &'a str> + Clone,
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
        run.extra_cargo_args.extend(
            controllers
                .clone()
                .into_iter()
                .flat_map(|c| ["--features", c]),
        );
        let mut m = controllers.into_iter().peekable();
        if let Some(c) = m.next() {
            if m.peek().is_none() {
                run.controller = Some(c);
            }
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

fn diff_analyzed(
    current_idx: usize,
    range: Rc<Vec<String>>,
    target_path: &Path,
) -> impl Fn(&Context, &mut RunMeasurements) {
    let current = range[current_idx].clone();
    let target_path = target_path.to_owned();
    let code_path = move |commit: &str| target_path.join(format!("{commit}.code.rs"));
    let current_code_path = code_path(&current);
    move |ctx, measurement| {
        ctx.write_analyzed_code(File::create(&current_code_path).unwrap(), false)
            .unwrap();
        for predecessor in (current_idx != 0)
            .then(|| &range[0..current_idx])
            .unwrap_or(&[])
            .iter()
            .rev()
        {
            let path = code_path(&predecessor);
            if !path.exists() {
                continue;
            }
            let diff = Command::new("diff")
                .args([
                    OsStr::new("-u"),
                    path.as_os_str(),
                    current_code_path.as_os_str(),
                ])
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            measurement.add_changed_lines(
                BufReader::new(diff.stdout.unwrap())
                    .lines()
                    .filter_map(|l| {
                        let l = l.unwrap();
                        ((l.starts_with('-') || l.starts_with('+'))
                            && !l.starts_with("---")
                            && !l.starts_with("+++"))
                        .then_some(())
                    })
                    .count() as u32,
            );
            break;
        }
    }
}

const ATOMIC_DEFAULT_CONTROLLERS: &[&str] = &[];
const FREEDIT_DEFAULT_CONTROLLERS: &[&str] = &[
    "edit-post-post",
    "comment-post",
    "solo-post",
    "user-chron-job",
];
const PLUME_DEFAULT_CONTROLLERS: &[&str] = &[];
const WEBUSUBMIT_DEFAULT_CONTROLLERS: &[&str] = &[
    "answers-controller",
    "forget-user",
    "questions-submit-internal",
];
const CONTILE_DEFAULT_CONTROLLERS: &[&str] = &[];
const HYPERSWITCH_DEFAULT_CONTROLLERS: &[&str] = &[
    "create-api-key",
    "payments-authorize-data",
    "setup-mandate-router-data",
];

fn get_all_commits(path: impl AsRef<Path>, start: &str, end: &str) -> Vec<String> {
    let output = Command::new("git")
        .args(["log", &format!("{end}^..{start}^"), "--format=%H"])
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
            Application::Contile { .. } => &[
                (PolicyResult::Pass, &[]),
                (PolicyResult::Fail, &["--features", "leak"]),
            ],
        }
    }

    pub fn all_controllers(&self) -> &'static [&'static str] {
        match self {
            Application::AtomicData { .. } => ATOMIC_DEFAULT_CONTROLLERS,
            Application::Lemmy { .. } => &[],
            Application::Plume { .. } => PLUME_DEFAULT_CONTROLLERS,
            Application::Websubmit { .. } => WEBUSUBMIT_DEFAULT_CONTROLLERS,
            Application::Contile { .. } => CONTILE_DEFAULT_CONTROLLERS,
            Application::Hyperswitch { .. } => HYPERSWITCH_DEFAULT_CONTROLLERS,
            Application::Freedit { .. } => FREEDIT_DEFAULT_CONTROLLERS,
        }
    }

    fn policies<'a>(&'a self) -> impl Iterator<Item = (&'a str, PolicyFn<'a>, Vec<&'a str>)> {
        match self {
            Application::AtomicData => Box::new(std::iter::once((
                "check-writes",
                Rc::new(atomic::check_rights) as PolicyFn<'a>,
                ATOMIC_DEFAULT_CONTROLLERS.to_owned(),
            )))
                as Box<dyn Iterator<Item = (&'a str, PolicyFn<'a>, Vec<&'a str>)>>,
            Application::Freedit { policies } => {
                Box::new(selection_or_all(policies).iter().map(|p| {
                    (
                        p.as_ref(),
                        Rc::new(move |ctx| p.check(ctx)) as PolicyFn<'a>,
                        FREEDIT_DEFAULT_CONTROLLERS.to_vec(),
                    )
                }))
            }
            Application::Hyperswitch { policies } => {
                Box::new(selection_or_all(policies).iter().map(|p| {
                    (
                        p.as_ref(),
                        Rc::new(p.runnable()) as PolicyFn<'a>,
                        HYPERSWITCH_DEFAULT_CONTROLLERS.to_vec(),
                    )
                }))
            }
            Application::Lemmy { policies, .. } => {
                Box::new(selection_or_all(policies).iter().map(|p| {
                    (
                        p.as_ref(),
                        Rc::new(move |cx| p.run(cx)) as PolicyFn<'a>,
                        vec!["all-controllers"],
                    )
                }))
            }
            Application::Plume => Box::new(std::iter::once((
                "data-deletion",
                Rc::new(plume::check) as PolicyFn<'a>,
                PLUME_DEFAULT_CONTROLLERS.to_vec(),
            ))),
            Application::Websubmit { policies, flavour } => {
                Box::new(selection_or_all(policies).iter().map(|p| {
                    (
                        p.as_ref(),
                        Rc::from(p.runnable(*flavour)) as PolicyFn<'a>,
                        WEBUSUBMIT_DEFAULT_CONTROLLERS.to_vec(),
                    )
                }))
            }
            Application::Contile { policies } => {
                Box::new(selection_or_all(policies).iter().map(|p| {
                    (
                        p.as_ref(),
                        Rc::from(p.runnable()) as PolicyFn<'a>,
                        CONTILE_DEFAULT_CONTROLLERS.to_vec(),
                    )
                }))
            }
        }
    }
}

impl Run<'_> {
    pub fn compile_cmd(&self) -> SPDGGenCommand {
        let app_config = self.app_config;
        let mut compile_cmd = SPDGGenCommand::global();
        if let Some(path) = self.external_annotations.or(app_config
            .external_annotations
            .as_ref()
            .map(|pb| pb.as_path()))
        {
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
