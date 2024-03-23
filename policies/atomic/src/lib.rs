use anyhow::Result;
use paralegal_policy::{
    assert_error,
    paralegal_spdg::{traverse::EdgeSelection, GlobalNode, Identifier, NodeCluster},
    Context, Diagnostics, Marker,
};
use std::sync::Arc;

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

        let new_resources = ctx
            .influencees(&commit, EdgeSelection::Data)
            .filter(|n| ctx.has_marker(marker!(new_resource), *n))
            .collect::<Box<[_]>>();

        // All checks that flow from the commit but not from a new_resource
        let valid_checks = ctx
            .influencees(&commit, EdgeSelection::Data)
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
