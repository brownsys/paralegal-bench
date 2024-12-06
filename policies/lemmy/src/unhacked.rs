use std::sync::Arc;

use anyhow::Result;
use paralegal_policy::{
    assert_error, assert_warning,
    paralegal_spdg::{Identifier, NodeCluster},
    Context, Diagnostics, EdgeSelection, NodeExt, NodeQueries, PolicyContext,
};

pub fn check_instance(ctx: Arc<PolicyContext>) -> Result<()> {
    let marker_instance = Identifier::new_intern("instance");
    let marker_instance_safe = Identifier::new_intern("instance_safe");
    let marker_instance_delete_check = Identifier::new_intern("instance_delete_check");
    let marker_instance_ban_check = Identifier::new_intern("instance_ban_check");

    let mut access_seen = false;
    for ctx in ctx.controller_contexts() {
        let accesses = ctx
            .nodes_marked_any_way(marker_instance)
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
    assert_warning!(
        ctx,
        access_seen,
        "VACUITY: No access seen in any controller"
    );

    Ok(())
}

pub fn check_community(ctx: Arc<PolicyContext>) -> Result<()> {
    let marker_community = Identifier::new_intern("community");
    let marker_community_delete_check = Identifier::new_intern("community_delete_check");
    let marker_community_ban_check = Identifier::new_intern("community_ban_check");
    let mut access_seen = false;

    for ctx in ctx.controller_contexts() {
        let accesses = ctx
            .nodes_marked_any_way(marker_community)
            .collect::<Box<_>>();
        if accesses.is_empty() {
            continue;
        }
        access_seen = true;

        let Some(delete_checks) =
            NodeCluster::try_from_iter(ctx.nodes_marked_any_way(marker_community_delete_check))
        else {
            ctx.error("No community delete checks found");
            continue;
        };
        let Some(ban_checks) =
            NodeCluster::try_from_iter(ctx.nodes_marked_any_way(marker_community_ban_check))
        else {
            ctx.error("No community ban checks found");
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
                ctx.node_error(access, "Unprotected access (community delete)");
            }
            if !ban_checks.flows_to(access, &ctx, EdgeSelection::Both) {
                ctx.node_error(access, "Unprotected access (community ban)");
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
