//! Conversion from input types to run configurations / run preparation

use clap::ValueEnum;
use lemmy::eval_driver::GetUserVersion;
use paralegal_policy::SPDGGenCommand;

use crate::input::{Application, EvaluationConfig, ExperimentConfig, ExperimentMode};
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
        self.experiments.iter().flat_map(|e| e.as_experiments(self))
    }
}

impl ExperimentConfig {
    pub fn as_experiments<'a>(
        &'a self,
        config: &'a EvaluationConfig,
    ) -> Box<dyn Iterator<Item = Run<'a>> + 'a> {
        match self.r#type {
            ExperimentMode::CaseStudy => match &self.application {
                Application::Lemmy { policies } => {
                    Box::new(self.lemmy_case_study(config, selection_or_all(policies)))
                        as Box<dyn Iterator<Item = _> + 'a>
                }
                Application::AtomicData => Box::new(self.atomic_case_study(config)),
                Application::Freedit { policies } => {
                    Box::new(self.freedit_case_study(config, selection_or_all(policies)))
                }
                Application::Hyperswitch { policies } => {
                    Box::new(self.hyperwitch_case_study(config, selection_or_all(policies)))
                }
                Application::WebSubmit { policies } => {
                    Box::new(self.websubmit_case_study(config, selection_or_all(policies)))
                }
                Application::Plume => Box::new(self.plume_case_study(config)),
            },
            _ => unimplemented!(),
        }
    }

    fn plume_case_study<'a>(
        &'a self,
        config: &'a EvaluationConfig,
    ) -> impl Iterator<Item = Run<'a>> {
        [
            (false, &[] as &[&str]),
            (true, &["--features", "plume-models/delete-comments"]),
        ]
        .into_iter()
        .map(|(expectation, extra_args)| {
            let mut exp =
                self.make_experiment(config, "plume", Box::new(plume::check), expectation);
            exp.compile_cmd.get_command().args(extra_args);
            exp
        })
    }

    fn websubmit_case_study<'a>(
        &'a self,
        config: &'a EvaluationConfig,
        policies: &'a [websubmit::Policy],
    ) -> impl Iterator<Item = Run<'a>> {
        unimplemented!() as std::vec::IntoIter<_>
    }

    fn hyperwitch_case_study<'a>(
        &'a self,
        config: &'a EvaluationConfig,
        policies: &'a [hyperswitch::Policy],
    ) -> impl Iterator<Item = Run<'a>> {
        policies.into_iter().flat_map(move |policy| {
            // Does not have a buggy version but I'm keeping this loop if we
            // want to add one
            [(true, &[] as &[&str])]
                .into_iter()
                .map(move |(expectation, _)| {
                    self.make_experiment(
                        config,
                        policy.as_ref(),
                        Box::new(policy.runnable()),
                        expectation,
                    )
                })
        })
    }

    fn freedit_case_study<'a>(
        &'a self,
        config: &'a EvaluationConfig,
        policies: &'a [freedit::Policy],
    ) -> impl Iterator<Item = Run<'a>> {
        policies.into_iter().flat_map(move |policy| {
            [(true, &[] as &[_]), (false, &["--features", "buggy"])]
                .into_iter()
                .map(move |(expectation, extra_args)| {
                    let mut exp = self.make_experiment(
                        config,
                        policy.as_ref(),
                        Box::new(move |ctx| policy.check(ctx)),
                        expectation,
                    );
                    exp.compile_cmd.get_command().args(extra_args);
                    exp
                })
        })
    }

    fn atomic_case_study<'a>(
        &'a self,
        config: &'a EvaluationConfig,
    ) -> impl Iterator<Item = Run<'a>> {
        [(true, &["--features", "bug-fix"] as &[_]), (false, &[])]
            .into_iter()
            .map(|(expectation, extra_args)| {
                let mut exp = self.make_experiment(
                    config,
                    "atomic",
                    Box::new(atomic::check_rights),
                    expectation,
                );
                exp.compile_cmd.get_command().args(extra_args);
                exp
            })
    }

    fn lemmy_case_study<'a>(
        &'a self,
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
                    ($expectation:expr, $controllers:expr) => {
                        $controllers.iter().map(move |c| {
                            let mut exp = self.make_experiment(
                                config,
                                policy_name,
                                Box::new(policy),
                                $expectation,
                            );
                            exp.comment = Some(c);
                            exp.compile_cmd.get_command().args(["--features", c]);
                            exp
                        })
                    };
                }
                batch_config
                    .baseline_controllers
                    .iter()
                    .flat_map(move |ctrl| mk_batch_exps!(!batch_config.expect_failure, ctrl))
                    .chain(batch_config.change.iter().flat_map(move |change| {
                        change
                            .affected_controllers
                            .into_iter()
                            .flat_map(move |c| mk_batch_exps!(!batch_config.expect_failure, c))
                    }))
                    .chain(batch_config.change.iter().flat_map(move |change| {
                        change
                            .affected_controllers
                            .iter()
                            .flat_map(move |c| mk_batch_exps!(batch_config.expect_failure, c))
                    }))
            })
    }

    fn make_experiment<'a>(
        &'a self,
        config: &'a EvaluationConfig,
        policy_name: &'a str,
        policy: PolicyFn<'a>,
        expectation: bool,
    ) -> Run<'a> {
        let app_config = &config.app_config[self.application.as_ref()];
        let mut compile_cmd = SPDGGenCommand::global();
        if let Some(path) = app_config.external_annotations.as_ref() {
            compile_cmd.external_annotations(path);
        }
        if app_config.abort {
            compile_cmd.abort_after_analysis();
        }
        compile_cmd
            .get_command()
            .args(app_config.flow_args.iter())
            .arg("--adaptive-depth")
            .arg("--")
            .args(app_config.cargo_args.iter());
        Run {
            config: self,
            policy_name,
            app_config,
            policy,
            expectation,
            compile_cmd,
            prepare: None,
            comment: None,
        }
    }
}
