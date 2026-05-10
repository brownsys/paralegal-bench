extern crate anyhow;
use std::{collections::HashSet, ops::Deref, path::Path, sync::Arc};

use anyhow::Result;
use paralegal_policy::{
    assert_error,
    diagnostics::ControllerContext,
    loc,
    paralegal_pdg::{self as paralegal_spdg, Endpoint as ControllerId, FunctionCallInfo, InstructionKind, TypeId},
    Context, Diagnostics, IntoIterGlobalNodes, Marker, NodeExt as _, NodeQueries,
    PolicyContext, RootContext,
};
use paralegal_spdg::{traverse::EdgeSelection, GlobalNode, Identifier};
use petgraph::{csr, visit::EdgeRef};
use serde::{Deserialize, Serialize};

macro_rules! marker {
    ($id:ident) => {
        Marker::new_intern(stringify!($id))
    };
}

pub const DEFAULT_CONTROLLERS: &[&str] = &[
    "answers-controller",
    "forget-user",
    "questions-submit-internal",
];

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
    fn unconditional(self, ctx: &RootContext) -> bool;
    fn is_return(self, ctx: &RootContext) -> bool;
    fn ctrl_influencer(self, ctx: &RootContext) -> Option<GlobalNode>;
}

impl NodeExt for GlobalNode {
    fn unconditional(self, ctx: &RootContext) -> bool {
        self.ctrl_influencer(ctx).is_none()
    }

    fn ctrl_influencer(self, ctx: &RootContext) -> Option<GlobalNode> {
        ctx.desc().controllers[&self.controller_id()]
            .graph
            .edges_directed(self.local_node(), petgraph::Incoming)
            .find(|e| e.weight().is_control())
            .map(|e| GlobalNode::from_local_node(self.controller_id(), e.source()))
    }

    fn is_return(self, ctx: &RootContext) -> bool {
        ctx.desc().controllers[&self.controller_id()]
            .return_
            .contains(&self.local_node())
    }
}

trait ContextExt {
    fn all_returns(&self) -> Box<dyn Iterator<Item = GlobalNode> + '_>;
}

impl ContextExt for RootContext {
    fn all_returns(&self) -> Box<dyn Iterator<Item = GlobalNode> + '_> {
        Box::new(self.desc().controllers.iter().flat_map(|(id, spdg)| {
            spdg.return_
                .iter()
                .copied()
                .map(move |local_node| GlobalNode::from_local_node(*id, local_node))
        }))
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
        assert!(!self.cx.desc().controllers.is_empty());

        let expect_delete = ctx.nodes_marked_any_way(stores).any(|store| {
            ctx.influencers(store, EdgeSelection::Data)
                .any(|sens| ctx.has_marker(sensitive, sens))
        });

        let cleaned = self.cx.controller_contexts().find(|ctx| {
            ctx.marked_nodes(from_storage).any(|src| {
                ctx.influencees(src, EdgeSelection::Data)
                    .any(|clean| ctx.has_marker(deletes, clean))
            })
        });

        if let Some(cleanup) = cleaned {
            cleanup.note(format!(
                "Found controller {} deletes stored data",
                cleanup.current().name
            ));
        } else if expect_delete {
            self.cx
                .error(format!("Found no controller that deletes data.",))
        };
        Ok(())
    }

    fn check_deletion_flow(&self, src: GlobalNode) -> Option<GlobalNode> {
        let delete = marker!(deletes);
        let m_no_skip = marker!(no_skip);
        let mut msg = self.struct_node_help(src, "Checking this node as a deleter");
        let r = match self.flavour {
            Flavour::Strict => {
                let ahb = self
                    .cx
                    .always_happens_before(
                        [src],
                        |c| {
                            if c != src
                                && !self.has_marker(delete, c)
                                && self.cx.instruction_at_node(c).kind.is_function_call()
                                && !self.cx.has_marker(m_no_skip, c)
                            {
                                msg.with_node_note(
                                    c,
                                    format!(
                                        "This is a checkpoint {}",
                                        self.node_info(c)
                                    ),
                                );
                                true
                            } else {
                                false
                            }
                        },
                        |c| self.cx.has_marker(delete, c),
                    )
                    .unwrap();
                let reached = ahb.reached().unwrap();
                if reached.is_empty() {
                    self.cx.node_note(
                        src,
                        format!(
                            "{} was a valid root, but did not reach delete",
                            src.info(&self.cx)
                        ),
                    );
                    msg.emit();
                    return None;
                }
                Some(reached.first()?.1)
            }
            _ => {
                self.cx
                    .influencees(src, EdgeSelection::Data)
                    // A node with marker "deletes"
                    .find(|influencee| self.cx.has_marker(delete, *influencee))
            }
        };
        msg.emit();
        r
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
                        .find_map(|&ty| self.find_deleter(ty, ctrl_id))
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

    fn find_deleter(
        &self,
        ty: TypeId,
        ctrl_id: ControllerId,
    ) -> Option<(TypeId, GlobalNode, GlobalNode)> {
        let auth = marker!(auth_witness);
        let mut sources: Box<dyn Iterator<Item = _>> = if self.flavour.is_strict() {
            Box::new(
                self.cx
                    .roots_where(|n: GlobalNode| n.has_type(ty, &self.cx))
                    .filter(|r| {
                        let is_ctrl_influenced = r.ctrl_influencer(&self.cx).is_some();
                        let is_auth_influenced = r
                            .influencers(&self.cx, EdgeSelection::Data)
                            .into_iter()
                            .any(|n| self.cx.has_marker(auth, n));
                        // self.cx.node_note(
                        //     *r,
                        //     format!(
                        //         "{} is {}ctrl influenced and {}auth influenced",
                        //         r.info(&self.cx).description,
                        //         if is_ctrl_influenced { "" } else { "not " },
                        //         if is_auth_influenced { "" } else { "not " },
                        //     ),
                        // );
                        !is_ctrl_influenced && is_auth_influenced
                    }),
            )
        } else {
            Box::new(self.cx.srcs_with_type(ctrl_id, ty))
        };
        let (from, to) = sources.find_map(|node| {
            let to = self.check_deletion_flow(node)?;
            Some((node, to))
        })?;
        Some((ty, from, to))
    }

    fn all_scopes(
        &self,
        target: GlobalNode,
        m_scopes: Marker,
    ) -> Box<dyn Iterator<Item = GlobalNode> + '_> {
        let cx = &self.cx;
        let c_id = target.controller_id();
        let store_cs = cx.node_info(target).at;
        let current = &cx.desc().controllers[&c_id];
        let m_request_gen = Identifier::new_intern("request_generated");
        let safe_source = marker!(safe_source);
        let scopes = cx
            .all_nodes_for_ctrl(c_id)
            .filter(move |node| self.cx.has_marker(m_scopes, *node));
        let mut direct_scopes = scopes
            .filter(move |n| cx.node_info(*n).at == store_cs)
            .peekable();
        match self.flavour {
            Flavour::Strict => {
                if current.return_.contains(&target.local_node()) {
                    Box::new(
                        cx.marked_type(safe_source)
                            .iter()
                            .flat_map(move |&t| cx.srcs_with_type(c_id, t)),
                    )
                } else {
                    Box::new(direct_scopes)
                }
            }
            Flavour::Lib => {
                if direct_scopes.peek().is_some() {
                    Box::new(direct_scopes)
                } else if current.return_.contains(&target.local_node()) {
                    Box::new(
                        current
                            .arguments
                            .iter()
                            .map(move |&n| GlobalNode::from_local_node(c_id, n))
                            .filter(move |n| n.has_marker(&self.cx, m_request_gen)),
                    )
                } else {
                    Box::new(
                        cx.influencers(target, EdgeSelection::Data)
                            .filter(move |n| cx.has_marker(m_scopes, *n)),
                    )
                }
            }
            Flavour::Application => Box::new(direct_scopes),
        }
    }

    fn scoped_storage_check_store(
        &self,
        cx: &Arc<ControllerContext>,
        sens: GlobalNode,
        store: GlobalNode,
        scopes: &[GlobalNode],
        witness_marker: Marker,
        witnesses: &[GlobalNode],
        found_local_witnesses: &mut bool,
    ) -> bool {
        let benign_marker = Identifier::new_intern("benign");
        // sensitive flows to store implies some scope flows to store callsite
        if !cx.flows_to(sens, store, EdgeSelection::Data) {
            return true;
        }
        let eligible_scopes = self
            .all_scopes(store, marker!(scopes_store))
            .collect::<Box<[_]>>();
        let strict_selection = |n: GlobalNode| {
            !n.has_marker(&cx, benign_marker)
                && !eligible_scopes.contains(&n)
                && matches!(
                    n.instruction(&cx).kind,
                    InstructionKind::FunctionCall(FunctionCallInfo { id, ..})
                    if !cx.desc()
                        .def_info[&id]
                        .markers
                        .iter()
                        .any(|m| m.marker == benign_marker)
                )
        };
        let mut checkpoints = HashSet::new();
        let holds = if self.flavour.is_strict() {
            let ahb = cx
                .always_happens_before(
                    cx.nodes_marked_any_way(witness_marker),
                    |n| {
                        let res = strict_selection(n);
                        if res {
                            checkpoints.insert(n);
                        }
                        res
                    },
                    |n| eligible_scopes.contains(&n),
                )
                .unwrap();
            !ahb.is_vacuous() && !ahb.holds()
        } else {
            let res = eligible_scopes.iter().find_map(|&scope| {
                cx.influencers(scope, EdgeSelection::Data)
                    .chain(std::iter::once(scope))
                    .find(|i| self.cx.has_marker(witness_marker, *i))
            });
            // if let Some(protect) = res {
            //     let mut msg = self.cx.struct_node_help(store, "This store is protected");
            //     msg.with_node_note(sens, "Stores this sensitive value");
            //     msg.with_node_note(protect, "Reached by this authentication");
            //     msg.emit()
            // }
            res.is_some()
        };
        if holds {
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
        if self.flavour.is_strict() {
            for f in checkpoints {
                err.with_node_note(
                    f,
                    format!(
                        "This node is a disallowed modification {} at {}",
                        f.info(&cx),
                        f.instruction(&cx).description
                    ),
                );
            }
        }
        err.emit();
        false
    }

    pub fn check_scoped_storage(&self) -> Result<()> {
        let scopes_store = marker!(scopes_store);
        let stores = marker!(stores);
        let sensitive = marker!(sensitive);
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
                        witness_marker,
                        &witnesses,
                        &mut found_local_witnesses,
                    )
                })
            });

            assert_error!(
                cx,
                controller_valid,
                loc!("Violation detected for controller: {}"),
                cx.current().name
            );

            if !controller_valid {
                if scopes.is_empty() {
                    self.warning(loc!("No valid scopes were found"));
                }
                // for a in cx.current().arguments().iter_global_nodes() {
                //     self.note(format!("{}", cx.describe_node(a)));
                //     let types = cx.current().node_types(a.local_node());
                //     for t in types {
                //         self.note(format!("{}", &cx.desc().type_info[&t].rendering))
                //     }
                // }
            }
        }
        Ok(())
    }

    /// If sensitive data is released, the release must be scoped, and all inputs to the scope must be safe.
    pub fn check_authorized_disclosure(&self) -> Result<()> {
        if self.flavour.is_lib() {
            return self.check_authorized_disclosure_lib();
        }

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
            for s in &safe_scopes {
                self.cx.node_note(*s, "this is a safe scope");
            }
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
                        self.node_note(*sink, "This sink is not reached");
                        return false;
                    }

                    let sink_callsite = sink.siblings(self);

                    // scopes for the store
                    let store_scopes = self
                        .cx
                        .influencers(&sink_callsite, EdgeSelection::Data)
                        .filter(|n| self.cx.has_marker(marker!(scopes), *n))
                        .collect::<Vec<_>>();
                    if store_scopes.is_empty() {
                        self.node_error(*sink, loc!("Did not find any scopes for this sink"));
                    }

                    let mut holds = true;
                    for root in roots.iter().copied() {
                        let mut msg = self.struct_node_help(root, "Checking this root");
                        let safe_before_scope = self
                            .cx
                            .always_happens_before(
                                [root],
                                |n| {
                                    if safe_scopes.contains(&n) {
                                        //msg.with_node_note(n, "Reached this checkpoint");
                                        true
                                    } else {
                                        false
                                    }
                                },
                                |n| store_scopes.contains(&n),
                            )
                            .unwrap();
                        msg.emit();

                        safe_before_scope.report(self.cx.clone());
                        holds &= safe_before_scope.holds();
                    }

                    // all flows are safe before scope
                    !holds
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

    fn check_authorized_disclosure_lib(&self) -> Result<()> {
        let ctx = &self.cx;
        let m_sensitive = marker!(sensitive);
        let m_sink = marker!(sink);
        let m_scopes = marker!(scopes);
        let m_request_gen = marker!(request_generated);
        let m_server_state = marker!(server_state);
        let m_from_storage = marker!(from_storage);
        let m_safe_source = marker!(safe_source);

        let is_safe_source = |n| {
            ctx.has_marker(m_request_gen, n)
                || ctx.has_marker(m_server_state, n)
                || ctx.has_marker(m_from_storage, n)
                || ctx.has_marker(m_safe_source, n)
        };

        for src in ctx
            .nodes_marked_any_way(m_sensitive)
            .chain(ctx.nodes_marked_any_way(m_from_storage))
        {
            for sink in ctx
                .influencees(src, EdgeSelection::Data)
                .filter(|s| ctx.has_marker(m_sink, *s) || s.is_return(ctx))
            {
                for scope in self.all_scopes(sink, m_scopes) {
                    assert_error!(
                        ctx,
                        ctx.any_flows(
                            ctx.all_nodes()
                                .filter(|n| is_safe_source(*n))
                                .collect::<Box<[_]>>()
                                .as_ref(),
                            &[scope],
                            EdgeSelection::Both,
                        )
                        .is_some()
                    );
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
    pub fn runnable(self, flavour: Flavour) -> Box<dyn Fn(Arc<RootContext>) -> Result<()>> {
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

    pub fn short_name(self) -> &'static str {
        match self {
            Self::AuthorizedDisclosure => "dis",
            Self::ScopedStorage => "sc",
            Self::Deletion => "del",
        }
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
