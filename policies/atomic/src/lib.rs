use anyhow::Result;
use paralegal_policy::{
    assert_error,
    paralegal_spdg::{traverse::EdgeSelection, GlobalNode, Identifier, NodeCluster, SourceUse},
    Context, Diagnostics, IntoIterGlobalNodes, Marker, NodeExt as _, NodeQueries, RootContext,
};
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

impl NodeExt for GlobalNode {
    fn is_argument(self, ctx: &RootContext, num: u8) -> bool {
        ctx.desc().controllers[&self.controller_id()]
            .graph
            .edges_directed(self.local_node(), Outgoing)
            .any(|e| matches!(e.weight().source_use, SourceUse::Argument(n) if n == num))
    }
}

pub mod cnl {
    include!(concat!(env!("OUT_DIR"), "/check-rights-alt.rs"));
}

policy!(check_rights, ctx {
    let mut any_sink_reached = false;
    let check_rights = marker!(check_rights);
    let m_sink = marker!(sink);
    let m_new_resource = marker!(new_resource);
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
            .filter(|&s| s.has_marker(&ctx, m_sink))
            .collect::<Box<[_]>>();
        if stores.is_empty() {
            continue;
        }
        any_sink_reached = true;

        let commit_influencees = ctx.influencees(&commit, EdgeSelection::Data).collect::<HashSet<_>>();

        let new_resources = commit_influencees
            .iter()
            .copied()
            .filter(|n| n.has_marker(&ctx, m_new_resource))
            .collect::<Box<[_]>>();

        // All checks that flow from the commit but not from a new_resource
        let valid_checks = commit_influencees.iter().copied()
            .filter(|check| {
                check.has_marker(&ctx, check_rights)
                    && ctx.any_flows(&new_resources, &[*check], EdgeSelection::Data).is_none()
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
                        ctx.has_ctrl_influence(check, store)
                        .then_some(check)
                    }),
                )
            })
            .collect::<Box<[_]>>();

        for (store, check) in checks.iter() {
            if check.is_none() {
                ctx.node_error(*store, "This store is not protected");
                // let store_influencing = ctx.influencers(*store, EdgeSelection::Control).chain(
                //     ctx.influencers(*store, EdgeSelection::Control).flat_map(|i| ctx.influencers(i, EdgeSelection::Data))
                // ).collect::<HashSet<_>>();


                // let mut msg = ctx.struct_node_help(*store, "This store");
                // for influencer in store_influencing.iter().copied() {
                //     msg.with_node_note(influencer, "Is ctrl-influenced by this");
                // }
                // msg.emit();
                // for c in valid_checks.iter().copied() {
                //     let mut msg = ctx.struct_node_help(c, "This is a valid check");

                //     let check_influenced =
                //         ctx.influencees(c, EdgeSelection::Control).chain(
                //             ctx.influencees(c, EdgeSelection::Data).flat_map(|i| ctx.influencees(i, EdgeSelection::Control))
                //         ).collect::<HashSet<_>>();
                //     for i in check_influenced.iter().copied() {
                //         msg.with_node_note(i, "that ctrl-influences this node");
                //     }
                //     msg.emit();

                //     for i in store_influencing.intersection(&check_influenced) {
                //         ctx.node_help(*i, "This is where influence intersects");
                //     }

                //     for i in store_influencing.iter().copied() {
                //         let mut msg = ctx.struct_node_help(i, "This store influence intersects");
                //         let mut emit = false;
                //         for intersection in ctx.influencers(i, EdgeSelection::Data) {
                //             if check_influenced.contains(&intersection) {
                //                 msg.with_node_note(intersection, "via this intermediary");
                //                 emit = true;
                //             }
                //         }
                //         if emit {
                //             msg.emit();
                //         }
                //     }

                //     ctx.always_happens_before(Some(c), |_| false, |t| t == *store).unwrap().report(ctx.clone());

                // }
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
