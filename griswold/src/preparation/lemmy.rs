use std::{rc::Rc, slice};

use lemmy::eval_driver::{BatchConfig, GetUserVersion};

use crate::{
    input::{ControllerRunMode, PolicyMode, PolicyResult},
    run::Run,
};

use super::{no_policy, selection_or_all, RunBuilder};

const LEMMY_CONTROLLERS: &[&str] = &[
    "comment-like",
    "comment-mark-as-read",
    "comment-save",
    "comment-report-create",
    "comment-report-list",
    "comment-report-resolve",
    "community-add-mod",
    "community-ban",
    "community-block",
    "community-follow",
    "community-hide",
    "community-transfer",
    "notification-list-mentions",
    "notification-list-replies",
    "notification-mark-all-read",
    "notification-mark-mention-read",
    "notification-unread-count",
    "user-add-admin",
    "user-ban-person",
    "user-block",
    "user-change-password",
    "user-list-banned",
    "user-login",
    "user-report-count",
    "user-save-settings",
    "post-like",
    "post-lock",
    "post-mark-read",
    "post-save",
    "post-sticky",
    "post-report-create",
    "post-report-list",
    "post-report-resolve",
    "private-message-mark-read",
    "purge-comment",
    "purge-community",
    "purge-person",
    "purge-post",
    "registration-approve",
    "registration-list",
    "registration-unread-counts",
    "site-leave-admin",
    "site-mod-log",
    "site-resolve-object",
    "site-search",
    "comment-create",
    "comment-delete",
    "comment-list",
    "comment-read",
    "comment-remove",
    "comment-update",
    "community-create",
    "community-delete",
    "community-list",
    "community-read",
    "community-remove",
    "community-update",
    "post-create",
    "post-delete",
    "post-list",
    "post-read",
    "post-remove",
    "post-update",
    "private-message-create",
    "private-message-delete",
    "private-message-read",
    "private-message-update",
    "site-create",
    "site-read",
    "site-update",
    "user-delete",
    "user-read",
];

impl<'a> RunBuilder<'a> {
    pub(super) fn lemmy_case_study(
        self,
        policies: &'a [lemmy::Prop],
        bugs: &'a [GetUserVersion],
    ) -> impl Iterator<Item = Run<'a>> {
        let bugs = selection_or_all(bugs);
        bugs.iter()
            .map(|v| (v, v.to_config()))
            .filter(|(_, c)| policies.contains(&c.property))
            .flat_map(move |(bug, batch_config)| {
                self.lemmy_prepare_for_batch_confg(batch_config, bug)
            })
    }

    fn lemmy_prepare_for_batch_confg(
        self,
        batch_config: &'a BatchConfig<'static>,
        bug: &'a GetUserVersion,
    ) -> impl Iterator<Item = Run<'a>> {
        let preparer = BatchConfigPreparer {
            builder: self,
            batch_config,
            bug,
        };
        preparer.runs()
    }
}

#[derive(Copy, Clone)]
struct BatchConfigPreparer<'a> {
    builder: RunBuilder<'a>,
    batch_config: &'a BatchConfig<'static>,
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
        let (policy_name, policy_fn, _) = match self.builder.experiment_config.policy_mode {
            PolicyMode::Unified => self.builder.unified_policies(),
            PolicyMode::Separate => (prop.as_ref(), Rc::new(|ctx| prop.run(ctx)) as _, vec![]),
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
            [
                self.case_study_run(
                    slice::from_ref(&slice::from_ref(&ctrl)),
                    self.batch_config.expect_failure,
                    initial_extra_feature,
                ),
                self.case_study_run(
                    slice::from_ref(&slice::from_ref(&ctrl)),
                    !self.batch_config.expect_failure,
                    changed_extra_feature,
                ),
            ]
            .into_iter()
        };
        match self.builder.experiment_config.controller_run_mode {
            ControllerRunMode::All => Box::new(run_pair("all-controllers")) as Box<_>,
            ControllerRunMode::AllSeparate => Box::new(
                LEMMY_CONTROLLERS
                    .iter()
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
