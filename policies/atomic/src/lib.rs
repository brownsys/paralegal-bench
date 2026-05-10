use anyhow::Result;
use paralegal_policy::{
    assert_error,
    paralegal_pdg::{traverse::EdgeSelection, GlobalNode, Identifier, NodeCluster},
    Context, Diagnostics, IntoIterGlobalNodes, Marker, NodeQueries, RootContext,
};
use petgraph::visit::EdgeRef;
use petgraph::Direction::Outgoing;
use std::{collections::HashSet, sync::Arc};

pub const DEFAULT_CONTROLLERS: &[&str] = &[];

macro_rules! marker {
    ($name:ident) => {{
        lazy_static::lazy_static! {
            static ref MARKER: Marker = Identifier::new_intern(stringify!($name));
        }
        *MARKER
    }};
}

macro_rules! policy {
    ($name:ident $(,)? $context:ident $(,)? $code:block) => {
        pub fn $name(ctx: Arc<RootContext>) -> Result<()> {
            ctx.named_policy(Identifier::new_intern(stringify!($name)), |$context| $code)
        }
    };
}

trait NodeExt: Sized {
    fn is_argument(self, ctx: &RootContext, num: u8) -> bool;
}

trait ContextExt {
    fn marked_nodes<'a>(&'a self, marker: Marker) -> Box<dyn Iterator<Item = GlobalNode> + 'a>;

    fn determines_ctrl(&self, influencer: GlobalNode, target: GlobalNode) -> bool;
}

impl ContextExt for RootContext {
    fn marked_nodes<'a>(&'a self, marker: Marker) -> Box<dyn Iterator<Item = GlobalNode> + 'a> {
        Box::new(
            self.desc()
                .controllers
                .keys()
                .copied()
                .flat_map(move |k| self.all_nodes_for_ctrl(k))
                .filter(move |node| self.has_marker(marker, *node)),
        )
    }

    fn determines_ctrl(&self, influencer: GlobalNode, target: GlobalNode) -> bool {
        self.influencees(influencer, EdgeSelection::Data)
            .any(|inf| self.flows_to(inf, target, EdgeSelection::Control))
    }
}

impl NodeExt for GlobalNode {
    fn is_argument(self, ctx: &RootContext, num: u8) -> bool {
        let graph = &ctx.desc().controllers[&self.controller_id()].graph;
        graph
            .edges_directed(self.local_node(), Outgoing)
            .any(|e| matches!(graph[e.target()].is_arg, Some(n) if n as u8 == num))
    }
}

policy!(check_rights, ctx {
    let mut any_sink_reached = false;
    let check_rights = marker!(check_rights);
    for ctx in ctx.controller_contexts() {
        let commit = NodeCluster::new(
            ctx.id(),
            ctx.marked_nodes(marker!(commit))
                .filter(|n| n.controller_id() == ctx.id())
                .map(|n| n.local_node()),
        );

        // If commit is stored
        let stores = ctx
            .influencees(&commit, EdgeSelection::Both)
            .filter(|s| ctx.has_marker(marker!(sink), *s))
            .collect::<Box<[_]>>();
        if stores.is_empty() {
            continue;
        }
        any_sink_reached = true;

        let commit_influencees = ctx.influencees(&commit, EdgeSelection::Data).collect::<HashSet<_>>();

        let new_resources = commit_influencees
            .iter()
            .copied()
            .filter(|n| ctx.has_marker(marker!(new_resource), *n))
            .filter(|n| {
                // Hackery
                //
                // On one hand this is hacky beacuse we're selecting a specific
                // argument. This shold probably be done cleanly via markers. On
                // the other hand we're just checking that the first argument is
                // not form the commit (e.g. user-specified), which is not bad,
                // but really I think this should be a whitelisted source, such
                // as `urls::PARENT`, *but* we can't annotate constants so this
                // has to do.
                let argument_siblings = n.siblings(&ctx)
                    .iter_global_nodes()
                    .filter(|n| n.is_argument(&ctx, 1))
                    .collect::<Box<[_]>>();

                let valid = argument_siblings.iter().copied().any(|n| {
                        commit_influencees.contains(&n)
                    });
                // let mut msg = ctx.struct_node_help(*n, format!("This is a new resource, it has {} argument 1 siblings. It is {}problematic", argument_siblings.len(), if valid { "" } else {"un"}));
                // for sibling in argument_siblings.iter().copied() {
                //     msg.with_node_note(sibling, "This is an argument 1 sibling");
                // }
                // msg.emit();
                valid

            })
            .collect::<Box<[_]>>();

        // All checks that flow from the commit but not from a new_resource
        let valid_checks = commit_influencees.iter().copied()
            .filter(|check| {
                ctx.has_marker(check_rights, *check)
                    && if let Some((from, to)) = ctx
                        .any_flows(&new_resources, &[*check], EdgeSelection::Data) {
                            // let mut msg = ctx.struct_node_note(to, "This is could be a check but");
                            // msg.with_node_help(from, "it is influenced by this new_resource");
                            // msg.emit();
                            false
                        } else {
                            true
                        }
            })
            .collect::<Box<[_]>>();

        if valid_checks.is_empty() {
            ctx.warning("No valid checks");
        }

        let checks = stores
            .iter()
            .copied()
            .map(|store| {
                (
                    store,
                    valid_checks.iter().copied().find_map(|check| {
                        let store_cs = ctx
                            .successors(store)
                            .find(|cs| ctx.has_ctrl_influence(check, *cs))?;
                        Some((check, store_cs))
                    }),
                )
            })
            .collect::<Box<[_]>>();

        for (store, check) in checks.iter() {
            if check.is_none() {
                let store_influencing = ctx.influencers(*store, EdgeSelection::Control).chain(
                    ctx.influencers(*store, EdgeSelection::Control).flat_map(|i| ctx.influencers(i, EdgeSelection::Data))
                ).collect::<HashSet<_>>();

                ctx.node_error(*store, "This store is not protected");

                let mut msg = ctx.struct_node_help(*store, "This store");
                for influencer in store_influencing.iter().copied() {
                    msg.with_node_note(influencer, "Is ctrl-influenced by this");
                }
                msg.emit();
                for c in valid_checks.iter().copied() {
                    let mut msg = ctx.struct_node_help(c, "This is a valid check");

                    let check_influenced =
                        ctx.influencees(c, EdgeSelection::Control).chain(
                            ctx.influencees(c, EdgeSelection::Data).flat_map(|i| ctx.influencees(i, EdgeSelection::Control))
                        ).collect::<HashSet<_>>();
                    for i in check_influenced.iter().copied() {
                        msg.with_node_note(i, "that ctrl-influences this node");
                    }
                    msg.emit();

                    for i in store_influencing.intersection(&check_influenced) {
                        ctx.node_help(*i, "This is where influence intersects");
                    }

                    for i in store_influencing.iter().copied() {
                        let mut msg = ctx.struct_node_help(i, "This store influence intersects");
                        let mut emit = false;
                        for intersection in ctx.influencers(i, EdgeSelection::Data) {
                            if check_influenced.contains(&intersection) {
                                msg.with_node_note(intersection, "via this intermediary");
                                emit = true;
                            }
                        }
                        if emit {
                            msg.emit();
                        }
                    }

                    ctx.always_happens_before(Some(c), |_| false, |t| t == *store).unwrap().report(ctx.clone());

                }
            }
        }
    }
    assert_error!(
        ctx,
        any_sink_reached,
        "No sink was reached across controllers, the policy may be vacuous or the markers not correctly assigned/unreachable."
    );

    Ok(())
});
