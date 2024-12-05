use std::sync::Arc;

use anyhow::Result;
use paralegal_policy::{
    assert_error,
    paralegal_spdg::{Identifier, NodeCluster},
    Context, Diagnostics, EdgeSelection, NodeExt, NodeQueries,
};

pub fn check(ctx: Arc<Context>) -> Result<()> {
    let marker_db_access = Identifier::new_intern("db_access");
    let marker_instance_safe = Identifier::new_intern("instance_safe");
    let marker_instance_delete_check = Identifier::new_intern("instance_delete_check");
    let marker_instance_ban_check = Identifier::new_intern("instance_ban_check");

    let mut access_seen = false;
    for ctx in ctx.controller_contexts() {
        let accesses = ctx
            .nodes_marked_any_way(marker_db_access)
            .filter(|a| !a.has_marker(&ctx, marker_instance_safe))
            .collect::<Box<_>>();
        if accesses.is_empty() {
            continue;
        }
        access_seen = true;

        let Some(delete_checks) =
            NodeCluster::try_from_iter(ctx.nodes_marked_any_way(marker_instance_delete_check))
        else {
            ctx.error("No delete checks found");
            continue;
        };
        let Some(ban_checks) =
            NodeCluster::try_from_iter(ctx.nodes_marked_any_way(marker_instance_ban_check))
        else {
            ctx.error("No ban checks found");
            continue;
        };

        for &access in accesses.iter() {
            // This is what it should be!!!
            //
            // if !delete_checks.has_ctrl_influence(access, &ctx) {
            //     ctx.node_error(access, "Unprotected access (delete)");
            // }
            // if !ban_checks.has_ctrl_influence(access, &ctx) {
            //     ctx.node_error(access, "Unprotected access (ban)");
            // }

            if !delete_checks.flows_to(access, &ctx, EdgeSelection::Both) {
                ctx.node_error(access, "Unprotected access (delete)");
            }
            if !ban_checks.flows_to(access, &ctx, EdgeSelection::Both) {
                ctx.node_error(access, "Unprotected access (ban)");
            }
        }
    }

    assert_error!(
        ctx,
        access_seen,
        "VACUITY: No access seen in any controller"
    );
    Ok(())
}
