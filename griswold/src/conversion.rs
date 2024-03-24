//! Conversion from input types to run configurations / run preparation

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
            .flat_map(move |(name, es)| es.iter().flat_map(move |e| e.as_experiments(&name, self)))
    }
}

impl ExperimentConfig {
    pub fn as_experiments<'a>(
        &'a self,
        name: &'a str,
        config: &'a EvaluationConfig,
    ) -> impl Iterator<Item = Run<'a>> + 'a {
        match &self.mode {
            ExperimentMode::CaseStudy => Box::new(self.case_study_runs(name, config))
                as Box<dyn Iterator<Item = Run<'a>> + 'a>,
            ExperimentMode::Ablation {
                feature_space_success,
                feature_space_fail,
            } => Box::new(self.case_study_runs(name, config).flat_map(move |run| {
                let run_clone_1 = run.clone();
                feature_space_success
                    .iter()
                    .map(move |feature| {
                        let mut run_clone = run_clone_1.clone();
                        run_clone.extra_cargo_args.extend(["--features", feature]);
                        run_clone.expectation = Expectation::Pass;
                        run_clone
                    })
                    .chain(feature_space_fail.iter().map(move |feature| {
                        let mut run_clone = run.clone();
                        run_clone.expectation = Expectation::Fail;
                        run_clone.extra_cargo_args.extend(["--features", feature]);
                        run_clone
                    }))
            })),
            _ => unimplemented!(),
        }
    }

    fn case_study_runs<'a>(
        &'a self,
        name: &'a str,
        config: &'a EvaluationConfig,
    ) -> impl Iterator<Item = Run<'a>> {
        match &self.application {
            Application::Lemmy { policies } => {
                Box::new(self.lemmy_case_study(name, config, selection_or_all(policies)))
                    as Box<dyn Iterator<Item = _> + 'a>
            }
            Application::AtomicData => Box::new(self.atomic_case_study(name, config)),
            Application::Freedit { policies } => {
                Box::new(self.freedit_case_study(name, config, selection_or_all(policies)))
            }
            Application::Hyperswitch { policies } => {
                Box::new(self.hyperwitch_case_study(name, config, selection_or_all(policies)))
            }
            Application::WebSubmit { policies } => {
                Box::new(self.websubmit_case_study(name, config, selection_or_all(policies)))
            }
            Application::Plume => Box::new(self.plume_case_study(name, config)),
        }
    }

    fn plume_case_study<'a>(
        &'a self,
        name: &'a str,
        config: &'a EvaluationConfig,
    ) -> impl Iterator<Item = Run<'a>> {
        [
            (Expectation::Fail, vec![]),
            (
                Expectation::Pass,
                vec!["--features", "plume-models/delete-comments"],
            ),
        ]
        .into_iter()
        .map(|(expectation, extra_args)| {
            let mut exp =
                self.case_study_run(name, config, "plume", Rc::new(plume::check), expectation);
            exp.extra_cargo_args = extra_args;
            exp
        })
    }

    fn websubmit_case_study<'a>(
        &'a self,
        name: &'a str,
        config: &'a EvaluationConfig,
        policies: &'a [websubmit::Policy],
    ) -> impl Iterator<Item = Run<'a>> {
        policies.iter().map(|p| {
            self.case_study_run(
                name,
                config,
                p.as_ref(),
                Rc::new(p.runnable()),
                Expectation::Pass,
            )
        })
    }

    fn hyperwitch_case_study<'a>(
        &'a self,
        name: &'a str,
        config: &'a EvaluationConfig,
        policies: &'a [hyperswitch::Policy],
    ) -> impl Iterator<Item = Run<'a>> {
        policies.into_iter().flat_map(move |policy| {
            // Does not have a buggy version but I'm keeping this loop if we
            // want to add one
            [(Expectation::Pass, &[] as &[&str])]
                .into_iter()
                .map(move |(expectation, _)| {
                    self.case_study_run(
                        name,
                        config,
                        policy.as_ref(),
                        Rc::new(policy.runnable()),
                        expectation,
                    )
                })
        })
    }

    fn freedit_case_study<'a>(
        &'a self,
        name: &'a str,
        config: &'a EvaluationConfig,
        policies: &'a [freedit::Policy],
    ) -> impl Iterator<Item = Run<'a>> {
        policies.into_iter().flat_map(move |policy| {
            [
                (Expectation::Pass, vec![]),
                (Expectation::Fail, vec!["--features", "buggy"]),
            ]
            .into_iter()
            .map(move |(expectation, extra_args)| {
                let mut exp = self.case_study_run(
                    name,
                    config,
                    policy.as_ref(),
                    Rc::new(move |ctx| policy.check(ctx)),
                    expectation,
                );
                exp.extra_cargo_args = extra_args;
                exp
            })
        })
    }

    fn atomic_case_study<'a>(
        &'a self,
        name: &'a str,
        config: &'a EvaluationConfig,
    ) -> impl Iterator<Item = Run<'a>> {
        [
            (Expectation::Pass, vec!["--features", "bug-fix"]),
            (Expectation::Fail, vec![]),
        ]
        .into_iter()
        .map(|(expectation, extra_args)| {
            let mut exp = self.case_study_run(
                name,
                config,
                "atomic",
                Rc::new(atomic::check_rights),
                expectation,
            );
            exp.extra_cargo_args = extra_args;
            exp
        })
    }

    fn lemmy_case_study<'a>(
        &'a self,
        name: &'a str,
        config: &'a EvaluationConfig,
        policies: &'a [lemmy::Prop],
    ) -> impl Iterator<Item = Run<'a>> {
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
                                name,
                                config,
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

    fn case_study_run<'a>(
        &'a self,
        name: &'a str,
        config: &'a EvaluationConfig,
        policy_name: &'a str,
        policy: PolicyFn<'a>,
        expectation: Expectation,
    ) -> Run<'a> {
        let app_config = &config.app_config[self.application.as_ref()];
        Run {
            experiment_name: name,
            config: self,
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
