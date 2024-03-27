extern crate anyhow;
use std::{collections::HashSet, ops::Deref, path::Path, sync::Arc};

use anyhow::Result;
use paralegal_policy::{
    assert_error, diagnostics::ControllerContext, loc, paralegal_spdg, Context, Diagnostics,
    IntoIterGlobalNodes, Marker, PolicyContext,
};
use paralegal_spdg::{traverse::EdgeSelection, GlobalNode, Identifier};
use serde::{Deserialize, Serialize};

macro_rules! marker {
    ($id:ident) => {
        Marker::new_intern(stringify!($id))
    };
}

/// Asserts that there exists one controller which calls a deletion
/// function on every value (or an equivalent type) that is ever stored.
pub struct PropRunner {
    cx: Arc<PolicyContext>,
    flavour: Flavour,
}

impl Deref for PropRunner {
    type Target = PolicyContext;
    fn deref(&self) -> &Self::Target {
        self.cx.deref()
    }
}

trait NodeExt {
    fn unconditional(self, ctx: &Context) -> bool;
}

impl NodeExt for GlobalNode {
    fn unconditional(self, ctx: &Context) -> bool {
        !ctx.desc().controllers[&self.controller_id()]
            .graph
            .edges_directed(self.local_node(), petgraph::Incoming)
            .any(|e| e.weight().is_control())
    }
}

impl PropRunner {
    pub fn new(cx: Arc<PolicyContext>, flavour: Flavour) -> Self {
        Self { cx, flavour }
    }

    pub fn flows_to_no_skip(
        &self,
        from: impl IntoIterGlobalNodes,
        to: impl IntoIterGlobalNodes,
    ) -> bool {
        let target = to.iter_global_nodes().collect::<HashSet<_>>();
        let ahb = self
            .always_happens_before(
                from.iter_global_nodes(),
                |n| {
                    self.cx.instruction_at_node(n).kind.is_function_call()
                        && !self.cx.has_marker(Identifier::new_intern("safe"), n)
                },
                |t| target.contains(&t),
            )
            .unwrap();
        !ahb.holds()
    }

    fn check_deletion_lib(&self) -> Result<()> {
        let ctx = &self.cx;
        let sensitive = marker!(sensitive);
        let stores = marker!(stores);
        let deletes = marker!(deletes);
        let from_storage = marker!(from_storage);
        let num_types_to_expect = ctx
            .marked_nodes(sensitive)
            .chain(
                ctx.marked_type(sensitive)
                    .iter()
                    .flat_map(|&t| {
                        ctx.all_controllers()
                            .flat_map(move |(ctrl, _)| ctx.srcs_with_type(ctrl, t))
                    })
                    .filter(|sens| {
                        ctx.influencees(*sens, EdgeSelection::Data)
                            .any(|store| ctx.has_marker(stores, store))
                    }),
            )
            .count();

        let cleanup = self.cx.controller_contexts().find(|ctx| {
            let cleaned = ctx.marked_nodes(from_storage).filter(|&src| {
                ctx.influencees(src, EdgeSelection::Data)
                    .any(|clean| ctx.has_marker(deletes, clean))
            });
            cleaned.count() == num_types_to_expect
        });

        if let Some(cleanup) = cleanup {
            cleanup.note(format!(
                "Found controller {} deletes all stored data",
                cleanup.current().name
            ));
        } else {
            self.cx
                .error("Found no controller that deletes {num_types_to_expect} types")
        };
        Ok(())
    }

    pub fn check_deletion(&self) -> Result<()> {
        if self.flavour.is_lib() {
            return self.check_deletion_lib();
        }

        // All types marked "sensitive"
        let types_to_check = self
            .cx
            .marked_type(marker!(sensitive))
            .iter()
            .filter(|t| {
                {
                    // If there is any controller
                    self.cx.desc().controllers.keys().any(|ctrl_id| {
                        // Where a source of that type
                        self.cx.srcs_with_type(*ctrl_id, **t).any(|sens_src| {
                            // Has data influence on
                            self.cx
                                .influencees(sens_src, EdgeSelection::Data)
                                .any(|influencee| {
                                    // A node with marker "influences"
                                    self.cx.has_marker(marker!(stores), influencee)
                                })
                        })
                    })
                }
            })
            .collect::<Vec<_>>();
        let found_deleter = self.cx.desc().controllers.keys().find_map(|&ctrl_id| {
            // For all types to check
            let deleters = types_to_check
                .iter()
                .copied()
                .filter_map(|&ty| {
                    std::iter::once(&ty)
                        .chain(self.cx.otypes(ty).iter())
                        .find_map(|&ty| {
                            let (from, to) =
                                self.cx.srcs_with_type(ctrl_id, ty).find_map(|node| {
                                    if self.flavour.is_strict() && !node.unconditional(&self.cx) {
                                        return None;
                                    }
                                    // That has data flow influence on
                                    let to = self
                                        .cx
                                        .influencees(node, EdgeSelection::Data)
                                        // A node with marker "deletes"
                                        .find(|influencee| {
                                            self.cx.has_marker(marker!(deletes), *influencee)
                                        })?;
                                    Some((node, to))
                                })?;
                            Some((ty, from, to))
                        })
                    // If there is any src of that type
                })
                .collect::<Box<[_]>>();
            (deleters.len() == types_to_check.len()).then_some((ctrl_id, deleters))
        });

        if let Some((deleter, deletions)) = found_deleter.as_ref() {
            let mut msg = self.cx.struct_help(format!(
                "The function {} is found to delete all types",
                self.cx.desc().controllers[deleter].name
            ));
            for (ty, from, to) in deletions.iter() {
                msg.with_node_note(
                    *from,
                    format!(
                        "This node returns type {}",
                        &self.cx.desc().type_info[ty].rendering
                    ),
                );
                msg.with_node_note(*to, "Which is deleted here");
            }
            msg.emit();
        } else {
            let mut msg = self
                .cx
                .struct_error("Did not find valid deleter for all types");
            for ty in types_to_check {
                msg.with_note(format!(
                    "Expected deletion of {}",
                    &self.cx.desc().type_info[ty].rendering
                ));
            }
            msg.emit();
        }

        Ok(())
    }

    fn scoped_storage_check_store(
        &self,
        cx: &Arc<ControllerContext>,
        sens: GlobalNode,
        store: GlobalNode,
        scopes: &[GlobalNode],
        safe_source: Marker,
        scopes_store: Marker,
        witness_marker: Marker,
        witnesses: &[GlobalNode],
        found_local_witnesses: &mut bool,
    ) -> bool {
        // sensitive flows to store implies some scope flows to store callsite
        if !cx.flows_to(sens, store, EdgeSelection::Data) {
            return true;
        }
        let store_cs = cx.node_info(store).at;
        let direct_scopes = scopes
            .iter()
            .copied()
            .filter(|n| cx.node_info(*n).at == store_cs)
            .collect::<Box<[_]>>();
        let eligible_scopes = match self.flavour {
            Flavour::Strict => {
                if !direct_scopes.is_empty() {
                    direct_scopes
                } else if cx.current().return_.contains(&store.local_node()) {
                    cx.marked_type(safe_source)
                        .iter()
                        .flat_map(|&t| cx.srcs_with_type(cx.id(), t))
                        .collect()
                } else {
                    cx.influencers(store, EdgeSelection::Data)
                        .filter(|n| cx.has_marker(scopes_store, *n))
                        .collect()
                }
            }
            Flavour::Lib => {
                if !direct_scopes.is_empty() {
                    direct_scopes
                } else if cx.current().return_.contains(&store.local_node()) {
                    cx.current()
                        .arguments
                        .iter()
                        .map(|&n| GlobalNode::from_local_node(cx.id(), n))
                        .collect()
                } else {
                    cx.influencers(store, EdgeSelection::Data)
                        .filter(|n| cx.has_marker(scopes_store, *n))
                        .collect()
                }
            }
            Flavour::Application => direct_scopes,
        };
        if eligible_scopes.iter().any(|&scope| {
            cx.influencers(scope, EdgeSelection::Data)
                .chain(std::iter::once(scope))
                .any(|i| self.cx.has_marker(witness_marker, i))
        }) {
            return true;
        }
        let mut err = cx.struct_node_error(store, loc!("Sensitive value store is not scoped."));
        err.with_node_note(sens, loc!("Sensitive value originates here"));
        if eligible_scopes.is_empty() {
            err.with_warning(loc!("No scopes were found to flow to this node"));
            for &scope in scopes.iter() {
                err.with_node_help(scope, "This node would have been a valid scope");
            }
        } else {
            for &scope in eligible_scopes.iter() {
                err.with_node_help(scope, "This scope would have been eligible but is not influenced by an `auth_whitness`");
            }
            if witnesses.is_empty() {
                *found_local_witnesses = false;
                err.with_warning(format!("No local `{witness_marker}` sources found."));
            }
            for w in witnesses.iter().copied() {
                err.with_node_help(w, &format!("This is a local source of `{witness_marker}`"));
            }
        }
        err.emit();
        false
    }

    pub fn check_scoped_storage(&self) -> Result<()> {
        let scopes_store = marker!(scopes_store);
        let stores = marker!(stores);
        let sensitive = marker!(sensitive);
        let safe_source = marker!(safe_source);
        let mut found_local_witnesses = true;
        let witness_marker = if self.flavour.is_lib() {
            marker!(request_generated)
        } else {
            marker!(auth_witness)
        };
        for cx in self.cx.clone().controller_contexts() {
            let c_id = cx.id();
            let scopes = cx
                .all_nodes_for_ctrl(c_id)
                .filter(|node| self.cx.has_marker(scopes_store, *node))
                .collect::<Box<[_]>>();

            let stores = cx
                .all_nodes_for_ctrl(c_id)
                .filter(|node| self.cx.has_marker(stores, *node))
                .collect::<Vec<_>>();
            let mut sensitives = cx
                .all_nodes_for_ctrl(c_id)
                .filter(|node| self.cx.has_marker(sensitive, *node));

            let witnesses = cx
                .all_nodes_for_ctrl(c_id)
                .filter(|node| self.cx.has_marker(witness_marker, *node))
                .collect::<Vec<_>>();

            let controller_valid = sensitives.all(|sens| {
                stores.iter().all(|&store| {
                    self.scoped_storage_check_store(
                        &cx,
                        sens,
                        store,
                        &scopes,
                        safe_source,
                        scopes_store,
                        witness_marker,
                        &witnesses,
                        &mut found_local_witnesses,
                    )
                })
            });

            assert_error!(
                cx,
                controller_valid,
                format!(
                    loc!("Violation detected for controller: {}"),
                    cx.current().name
                ),
            );

            if !controller_valid {
                if scopes.is_empty() {
                    self.warning(loc!("No valid scopes were found"));
                }
                for a in cx.current().arguments().iter_global_nodes() {
                    self.note(format!("{}", cx.describe_node(a)));
                    let types = cx.current().node_types(a.local_node());
                    for t in types {
                        self.note(format!("{}", &cx.desc().type_info[&t].rendering))
                    }
                }
            }
        }
        Ok(())
    }

    /// If sensitive data is released, the release must be scoped, and all inputs to the scope must be safe.
    pub fn check_authorized_disclosure(&self) -> Result<()> {
        for c_id in self.cx.desc().controllers.keys() {
            // All srcs that have no influencers
            let roots = self
                .cx
                .roots(*c_id, EdgeSelection::Data)
                .collect::<Vec<_>>();

            let safe_scopes = self
                .cx
                // All nodes marked "safe"
                .all_nodes_for_ctrl(*c_id)
                .filter(|n| self.cx.has_marker(marker!(safe_source), *n))
                // And all nodes marked "safe_with_bless"
                .chain(self.cx.all_nodes_for_ctrl(*c_id).filter(|node| {
                    self.cx.has_marker(marker!(safe_source_with_bless), *node)
                        && self
                            .cx
                            // That are influenced by a node marked "bless"
                            .influencers(*node, EdgeSelection::Both)
                            .any(|b| self.cx.has_marker(marker!(bless_safe_source), b))
                }))
                .collect::<Vec<_>>();
            let sinks = self
                .cx
                .all_nodes_for_ctrl(*c_id)
                .filter(|n| self.cx.has_marker(marker!(sink), *n))
                .collect::<Vec<_>>();
            let mut sensitives = self
                .cx
                .all_nodes_for_ctrl(*c_id)
                .filter(|node| self.cx.has_marker(marker!(sensitive), *node));

            let some_failure = sensitives.any(|sens| {
                sinks.iter().any(|sink| {
                    // sensitive flows to store implies
                    if !self.cx.flows_to(sens, *sink, EdgeSelection::Data) {
                        return false;
                    }

                    let call_sites = self.cx.consuming_call_sites(*sink).collect::<Box<[_]>>();
                    let [cs] = call_sites.as_ref() else {
                        self.cx.node_error(
                            *sink,
                            format!(
                                "Unexpected number of call sites {} for this node",
                                call_sites.len()
                            ),
                        );
                        return false;
                    };
                    let sink_callsite = self.cx.inputs_of(*cs);

                    // scopes for the store
                    let store_scopes = self
                        .cx
                        .influencers(&sink_callsite, EdgeSelection::Data)
                        .filter(|n| self.cx.has_marker(marker!(scopes), *n))
                        .collect::<Vec<_>>();
                    if store_scopes.is_empty() {
                        self.node_error(*sink, loc!("Did not find any scopes for this sink"));
                    }

                    // all flows are safe before scope
                    let safe_before_scope = self
                        .cx
                        .always_happens_before(
                            roots.iter().cloned(),
                            |n| safe_scopes.contains(&n),
                            |n| store_scopes.contains(&n),
                        )
                        .unwrap();

                    safe_before_scope.report(self.cx.clone());

                    !safe_before_scope.holds()
                })
            });

            if some_failure {
                let mut nodes = self.marked_nodes(marker!(scopes)).peekable();
                if nodes.peek().is_none() {
                    let mut err = self.struct_help(loc!("No suitable scopes were found"));

                    for scope in nodes {
                        err.with_node_note(scope, "This location would have been a suitable scope");
                    }

                    err.emit();
                }
            }
        }
        Ok(())
    }
}

#[derive(Copy, Clone, clap::ValueEnum, strum::AsRefStr, Serialize, Deserialize)]
#[strum(serialize_all = "kebab-case")]
#[serde(rename_all = "kebab-case")]
pub enum Policy {
    ScopedStorage,
    Deletion,
    AuthorizedDisclosure,
}

impl Policy {
    pub fn runnable(self, flavour: Flavour) -> Box<dyn Fn(Arc<Context>) -> Result<()>> {
        Box::new(move |ctx| {
            ctx.named_policy(Identifier::new_intern(self.as_ref()), |ctx| {
                let runner = PropRunner::new(ctx, flavour);
                match self {
                    Policy::ScopedStorage => runner.check_scoped_storage(),
                    Policy::AuthorizedDisclosure => runner.check_authorized_disclosure(),
                    Policy::Deletion => runner.check_deletion(),
                }
            })
        })
    }
}

#[derive(Copy, Clone, clap::ValueEnum, strum::AsRefStr, Serialize, Deserialize, strum::EnumIs)]
#[strum(serialize_all = "kebab-case")]
#[serde(rename_all = "kebab-case")]
pub enum Flavour {
    Lib,
    Application,
    Strict,
}

impl Flavour {
    pub fn external_annotations(self) -> &'static Path {
        Path::new(match self {
            Flavour::Lib => "lib-external-annotations.toml",
            Flavour::Application => "baseline-external-annotations.toml",
            Flavour::Strict => "strict-external-annotations.toml",
        })
    }

    pub fn annotation_feature(self) -> &'static str {
        match self {
            Flavour::Application => "v-ann-baseline",
            Flavour::Lib => "v-ann-lib",
            Flavour::Strict => "v-ann-strict",
        }
    }
}
