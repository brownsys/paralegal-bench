use std::{rc::Rc, slice};

use lemmy::eval_driver::{BatchConfig, GetUserVersion, LemmyPackage};

use crate::{
    input::{ControllerRunMode, PolicyMode, PolicyResult},
    run::Run,
};

use super::{no_policy, selection_or_all, RunBuilder};

impl<'a> RunBuilder<'a> {
    pub(super) fn lemmy_case_study(
        self,
        policies: &'a [lemmy::Prop],
        bugs: &'a [GetUserVersion],
        new_version: Option<LemmyPackage>,
    ) -> impl Iterator<Item = Run<'a>> {
        let bugs = selection_or_all(bugs);
        bugs.iter()
            .map(|v| (v, v.to_config()))
            .filter(|(_, c)| policies.contains(&c.property))
            .flat_map(move |(bug, batch_config)| {
                self.lemmy_prepare_for_batch_confg(batch_config, bug, new_version)
            })
    }

    fn lemmy_prepare_for_batch_confg(
        self,
        batch_config: &'a BatchConfig<'static>,
        bug: &'a GetUserVersion,
        new_version: Option<LemmyPackage>,
    ) -> impl Iterator<Item = Run<'a>> {
        let preparer = BatchConfigPreparer {
            builder: self,
            batch_config,
            bug,
            new_version,
        };
        preparer.runs()
    }
}

#[derive(Copy, Clone)]
struct BatchConfigPreparer<'a> {
    builder: RunBuilder<'a>,
    batch_config: &'a BatchConfig<'static>,
    bug: &'a GetUserVersion,
    new_version: Option<LemmyPackage>,
}

impl<'a> BatchConfigPreparer<'a> {
    fn case_study_run(
        self,
        controller_features: &[&[&'a str]],
        expect_fail: bool,
        extra_feature: Option<&'static str>,
    ) -> Run<'a> {
        let prop = &self.batch_config.property;
        let new_version = self.new_version;
        let (policy_name, policy_fn, _) = match self.builder.experiment_config.policy_mode {
            PolicyMode::Unified => self.builder.unified_policies(),
            PolicyMode::Separate => (
                prop.as_ref(),
                Rc::new(move |ctx| prop.run(ctx, new_version.is_some(), false)) as _,
                vec![],
            ),
            PolicyMode::None => no_policy(),
        };
        let controllers = controller_features.iter().flat_map(|i| i.iter()).copied();
        let mut exp = self.builder.case_study_run(
            policy_name,
            policy_fn,
            if expect_fail {
                PolicyResult::Fail
            } else {
                PolicyResult::Pass
            },
            controllers,
        );
        exp.bug = Some(self.bug.as_ref());
        if let Some(package) = self.new_version {
            exp.extra_flow_args.extend(["--target", package.as_str()]);
        }
        exp.extra_cargo_args
            .extend(["--features", self.batch_config.baseline_feature]);
        if let Some(f) = extra_feature {
            exp.extra_cargo_args.extend(["--features", &f])
        }
        exp
    }

    fn mk_batch_exps(
        self,
        expect_fail: bool,
        controllers: &'a [&'static [&'static str]],
        extra_feature: Option<&'static str>,
    ) -> Box<dyn Iterator<Item = Run<'a>> + 'a> {
        match self.builder.experiment_config.controller_run_mode {
            ControllerRunMode::Affected => {
                let iter = controllers.iter().flat_map(|s| s.iter()).map(move |c| {
                    self.case_study_run(
                        slice::from_ref(&slice::from_ref(c)),
                        expect_fail,
                        extra_feature,
                    )
                });
                Box::new(iter)
            }
            ControllerRunMode::AffectedMerged => {
                // If this batch happens to be empty we must return no run.
                // Otherwise this can cause spuriously succeeding runs.
                if controllers.iter().flat_map(|s| s.iter()).next().is_none() {
                    Box::new(std::iter::empty())
                } else {
                    Box::new(std::iter::once(self.case_study_run(
                        controllers,
                        expect_fail,
                        extra_feature,
                    )))
                }
            }
            ControllerRunMode::All | ControllerRunMode::AllSeparate => unreachable!(),
        }
    }

    fn extra_features(self) -> (Option<&'static str>, Option<&'static str>) {
        if let Some(change) = self.batch_config.change.as_ref() {
            if change.add_feature {
                (None, Some(change.change_feature))
            } else {
                (Some(change.change_feature), None)
            }
        } else {
            (None, None)
        }
    }

    fn runs(self) -> Box<dyn Iterator<Item = Run<'a>> + 'a> {
        let (initial_extra_feature, changed_extra_feature) = self.extra_features();
        let change = &self.batch_config.change;
        let run_pair = move |ctrl| {
            let failing = || {
                self.case_study_run(
                    slice::from_ref(&slice::from_ref(&ctrl)),
                    true,
                    if self.batch_config.expect_failure {
                        initial_extra_feature
                    } else {
                        changed_extra_feature
                    },
                )
            };
            let succeeding = {
                self.case_study_run(
                    slice::from_ref(&slice::from_ref(&ctrl)),
                    !self.batch_config.expect_failure,
                    if self.batch_config.expect_failure {
                        changed_extra_feature
                    } else {
                        initial_extra_feature
                    },
                )
            };
            std::iter::once(succeeding)
                .chain((!self.builder.experiment_config.policy_mode.is_none()).then(failing))
        };
        match self.builder.experiment_config.controller_run_mode {
            ControllerRunMode::All => Box::new(run_pair("all-controllers")) as Box<_>,
            ControllerRunMode::AllSeparate => Box::new(
                matches!(self.new_version, None | Some(LemmyPackage::Api))
                    .then_some(lemmy::eval_driver::LEMMY_API_CONTROLLERS)
                    .into_iter()
                    .flatten()
                    .chain(
                        matches!(self.new_version, None | Some(LemmyPackage::ApiCrud))
                            .then_some(lemmy::eval_driver::LEMMY_API_CRUD_CONTROLLERS)
                            .into_iter()
                            .flatten(),
                    )
                    .copied()
                    .flat_map(move |c| run_pair(&c)),
            ),
            _ => {
                let iter = self
                    .mk_batch_exps(
                        self.batch_config.expect_failure,
                        self.batch_config.baseline_controllers,
                        initial_extra_feature,
                    )
                    .chain(change.iter().flat_map(move |change| {
                        change.affected_controllers.iter().flat_map(move |c| {
                            self.mk_batch_exps(
                                self.batch_config.expect_failure,
                                slice::from_ref(c),
                                initial_extra_feature,
                            )
                        })
                    }))
                    .chain(change.iter().flat_map(move |change| {
                        let fixed = change
                            .affected_controllers
                            .as_ref()
                            .map_or(self.batch_config.baseline_controllers, |ctrl| {
                                slice::from_ref(ctrl)
                            });
                        self.mk_batch_exps(
                            !self.batch_config.expect_failure,
                            fixed,
                            changed_extra_feature,
                        )
                    }));
                Box::new(iter)
            }
        }
    }
}
