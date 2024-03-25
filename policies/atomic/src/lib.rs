use anyhow::Result;
use paralegal_policy::{
    assert_error,
    paralegal_spdg::{traverse::EdgeSelection, GlobalNode, Identifier, NodeCluster, SourceUse},
    Context, Diagnostics, Marker,
};
use petgraph::{visit::EdgeRef, Direction::Outgoing};
use std::{collections::HashSet, sync::Arc};

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
        pub fn $name(ctx: Arc<Context>) -> Result<()> {
            ctx.named_policy(Identifier::new_intern(stringify!($name)), |$context| $code)
        }
    };
}

trait NodeExt: Sized {
    fn siblings(self, ctx: &Context) -> Box<dyn Iterator<Item = GlobalNode> + '_>;

    fn is_argument(self, ctx: &Context, num: u8) -> bool;
}

trait ContextExt {
    fn marked_nodes<'a>(&'a self, marker: Marker) -> Box<dyn Iterator<Item = GlobalNode> + 'a>;

    fn determines_ctrl(&self, influencer: GlobalNode, target: GlobalNode) -> bool;
}

impl ContextExt for Context {
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
    fn siblings(self, ctx: &Context) -> Box<dyn Iterator<Item = GlobalNode> + '_> {
        let mut set: HashSet<_> = ctx
            .predecessors(self)
            .flat_map(|n| ctx.successors(n))
            .chain(ctx.successors(self).flat_map(|n| ctx.predecessors(n)))
            .collect();
        set.remove(&self);
        Box::new(set.into_iter())
    }

    fn is_argument(self, ctx: &Context, num: u8) -> bool {
        ctx.desc().controllers[&self.controller_id()]
            .graph
            .edges_directed(self.local_node(), Outgoing)
            .any(|e| matches!(e.weight().source_use, SourceUse::Argument(n) if n == num))
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
                !n.siblings(&ctx)
                    .filter(|n| n.is_argument(&ctx, 1))
                    .all(|n| !commit_influencees.contains(&n))
            })
            .collect::<Box<[_]>>();

        // All checks that flow from the commit but not from a new_resource
        let valid_checks = commit_influencees.iter().copied()
            .filter(|check| {
                ctx.has_marker(check_rights, *check)
                    && ctx
                        .any_flows(&new_resources, &[*check], EdgeSelection::Data)
                        .is_none()
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
                ctx.node_error(*store, "This store is not protected");
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
