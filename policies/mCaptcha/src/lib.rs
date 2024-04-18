use anyhow::{Ok, Result};
use paralegal_policy::{
    assert_error, paralegal_spdg::traverse::EdgeSelection, Context, Diagnostics, Marker, NodeExt,
};
use std::sync::Arc;

#[derive(
    Copy,
    Clone,
    Eq,
    PartialEq,
    strum::AsRefStr,
    serde::Serialize,
    serde::Deserialize,
    clap::ValueEnum,
)]
#[strum(serialize_all = "kebab-case")]
#[serde(rename_all = "kebab-case")]
pub enum Policy {
    Deletion,
    OptInBeforeSave,
}

pub const DEFAULT_CONTROLLERS: &[&str] = &["verify-pow", "delete-account"];

impl Policy {
    pub fn runnable(self) -> fn(Arc<Context>) -> Result<()> {
        match self {
            Self::Deletion => deletion_policy as _,
            Self::OptInBeforeSave => verify_opt_in_before_save_policy,
        }
    }
}

#[allow(dead_code)]
fn deletion_policy(ctx: Arc<Context>) -> Result<()> {
    let user_data_types = ctx.marked_type(Marker::new_intern("user_data"));

    let found = ctx.all_controllers().any(|(deleter_id, _ignored)| {
        let delete_sinks = ctx
            .all_nodes_for_ctrl(deleter_id)
            .filter(|n| ctx.has_marker(Marker::new_intern("deletes"), *n))
            .collect::<Vec<_>>();
        user_data_types.iter().all(|&t| {
            let sources = ctx.srcs_with_type(deleter_id, t).collect::<Vec<_>>();
            ctx.any_flows(&sources, &delete_sinks, EdgeSelection::Data)
                .is_some()
        })
    });
    assert_error!(ctx, found, "Could not find a controller deleting all types");
    Ok(())
}

fn verify_opt_in_before_save_policy(ctx: Arc<Context>) -> Result<()> {
    ctx.all_controllers().all(|(c_id, _)| {
        let mut save_stats_to_db_nodes = ctx.all_nodes_for_ctrl(c_id).filter(|n| n.has_marker(&ctx, Marker::new_intern("save_stats_to_db")));
        let mut verify_opt_in_nodes = ctx.all_nodes_for_ctrl(c_id).filter(|n| n.has_marker(&ctx, Marker::new_intern("verify_opt_in")));
        let mut sources = ctx.all_nodes_for_ctrl(c_id).filter(|n| n.has_marker(&ctx, Marker::new_intern("site_key")));

        sources.all(|site_key|  {
            ctx.node_help(site_key, "this is a site key");
            save_stats_to_db_nodes.all(|sink|  {
                ctx.node_help(sink, "this is a sink");
                if ctx.influencers(sink, EdgeSelection::Both).any(|n| n == site_key) {
                    let verified = verify_opt_in_nodes.any(|verify_node| {
                        ctx.node_help(verify_node, "This is a verification");
                        ctx.influencers(verify_node, EdgeSelection::Data).any(|n| n == site_key)
                            && ctx.has_ctrl_influence(verify_node, sink)
                    });
                    if !verified {
                        let mut msg = ctx.struct_error("Save operation to DB for site_key must be preceded by its verification through verify_opt_in");
                        msg.with_node_note(site_key, "This is the site key");
                        msg.with_node_note(sink, "which gets saved here");
                        msg.emit();
                    }
                    verified
                } else {
                    true
                }
            })
        })
    });

    Ok(())
}
