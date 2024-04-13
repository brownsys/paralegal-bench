use std::{rc::Rc, slice};

use lemmy::eval_driver::{BatchConfig, GetUserVersion};

use crate::{
    input::{LemmyControllerRunMode, PolicyMode, PolicyResult},
    run::Run,
};

use super::{selection_or_all, RunBuilder};

impl<'a> RunBuilder<'a> {
    pub(super) fn lemmy_case_study(
        self,
        policies: &'a [lemmy::Prop],
        bugs: &'a [GetUserVersion],
        run_mode: LemmyControllerRunMode,
    ) -> impl Iterator<Item = Run<'a>> {
        let bugs = selection_or_all(bugs);
        bugs.iter()
            .map(|v| (v, v.to_config()))
            .filter(|(_, c)| policies.contains(&c.property))
            .flat_map(move |(bug, batch_config)| {
                self.lemmy_prepare_for_batch_confg(batch_config, run_mode, bug)
            })
    }

    fn lemmy_prepare_for_batch_confg(
        self,
        batch_config: &'a BatchConfig<'static>,
        run_mode: LemmyControllerRunMode,
        bug: &'a GetUserVersion,
    ) -> impl Iterator<Item = Run<'a>> {
        let preparer = BatchConfigPreparer {
            builder: self,
            batch_config,
            run_mode,
            bug,
        };
        preparer.runs()
    }
}

#[derive(Copy, Clone)]
struct BatchConfigPreparer<'a> {
    builder: RunBuilder<'a>,
    batch_config: &'a BatchConfig<'static>,
    run_mode: LemmyControllerRunMode,
    bug: &'a GetUserVersion,
}

impl<'a> BatchConfigPreparer<'a> {
    fn case_study_run(
        self,
        controller_features: &[&[&'a str]],
        expect_fail: bool,
        extra_feature: Option<&'static str>,
    ) -> Run<'a> {
        let prop = &self.batch_config.property;
        let (policy_name, policy_fn) = match self.builder.experiment_config.policy_mode {
            PolicyMode::Unified => self.builder.unified_policies(),
            PolicyMode::Separate => (prop.as_ref(), Rc::new(|ctx| prop.run(ctx)) as _),
        };
        let mut exp = self.builder.case_study_run(
            policy_name,
            policy_fn,
            if expect_fail {
                PolicyResult::Fail
            } else {
                PolicyResult::Pass
            },
        );
        exp.controller = match controller_features {
            [[f]] => Some(f),
            _ => None,
        };
        exp.bug = Some(self.bug.as_ref());
        exp.extra_cargo_args = controller_features
            .iter()
            .flat_map(|s| s.iter())
            .flat_map(|c| ["--features", c])
            .chain(["--features", self.batch_config.baseline_feature])
            .collect();
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
        match self.run_mode {
            LemmyControllerRunMode::Affected => {
                let iter = controllers.iter().flat_map(|s| s.iter()).map(move |c| {
                    self.case_study_run(
                        slice::from_ref(&slice::from_ref(c)),
                        expect_fail,
                        extra_feature,
                    )
                });
                Box::new(iter)
            }
            LemmyControllerRunMode::All => Box::new(std::iter::once(self.case_study_run(
                slice::from_ref(&slice::from_ref(&"all-controllers")),
                expect_fail,
                extra_feature,
            ))),
            LemmyControllerRunMode::AffectedMerged => Box::new(std::iter::once(
                self.case_study_run(controllers, expect_fail, extra_feature),
            )),
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
