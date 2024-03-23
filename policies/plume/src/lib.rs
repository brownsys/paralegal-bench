use anyhow::Result;
use clap::ValueEnum;
use std::sync::Arc;

use paralegal_policy::{paralegal_spdg::traverse::EdgeSelection, Context, Diagnostics, Marker};

macro_rules! marker {
    ($id:ident) => {
        Marker::new_intern(stringify!($id))
    };
}

pub fn check(ctx: Arc<Context>) -> Result<()> {
    let user_data_types = ctx.marked_type(marker!(user_data));

    let found = ctx.all_controllers().find(|(deleter_id, ctrl)| {
        let delete_sinks = ctx
            .all_nodes_for_ctrl(*deleter_id)
            .filter(|n| ctx.has_marker(marker!(to_delete), *n))
            .collect::<Vec<_>>();
        user_data_types.iter().all(|&t| {
            let sources = ctx.srcs_with_type(*deleter_id, t).collect::<Vec<_>>();
            if ctx
                .any_flows(&sources, &delete_sinks, EdgeSelection::Data)
                .is_none()
            {
                let mut note = ctx.struct_note(format!(
                    "The type {} is not being deleted in {}",
                    ctx.desc().type_info[&t].rendering,
                    ctrl.name
                ));
                for src in sources {
                    note.with_node_note(src, "This is a source for that type");
                }
                for snk in &delete_sinks {
                    note.with_node_note(*snk, "This is a potential delete sink");
                }
                note.emit();
                false
            } else {
                true
            }
        })
    });
    if found.is_none() {
        ctx.error("Could not find a function deleting all types");
    }
    if let Some((found, _)) = found {
        println!(
            "Found {} deletes all user data types",
            ctx.desc().controllers[&found].name
        );
        for t in user_data_types {
            println!("Found user data {}", ctx.describe_def(*t));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, ValueEnum, PartialOrd, Ord, PartialEq, Eq)]
#[clap(rename_all = "kebab-case")]
pub enum PlumeVersion {
    /// Original, Deletes no comments
    V0,
    /// Deleted comments
    V1,
    /// What the policy should be: requires media deletion
    V2,
    /// If the media deletion was fixed
    V3,
}
