extern crate anyhow;
extern crate clap;
extern crate paralegal_policy;

use clap::Parser;

use anyhow::{bail, Result};
use paralegal_policy::{
    assert_error, assert_warning,
    paralegal_spdg::{traverse::EdgeSelection, GlobalNode, Identifier, Node},
    Context, Diagnostics, GraphLocation, Marker,
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
        fn $name(ctx: Arc<Context>) -> Result<()> {
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
    let commits = ctx.marked_nodes(marker!(commit));
    let mut any_sink_reached = false;
    for commit in commits {
        let check_rights = marker!(check_rights);
        // If commit is stored
        let mut stores = ctx.influencees(commit, EdgeSelection::Both)
            .filter(|s| ctx.has_marker(marker!(sink), *s))
            .peekable();

        let mut stores = ctx
            // .all_nodes_for_ctrl(commit.controller_id())
            // .filter(|n| ctx.has_marker(marker!(sink), *n))
            .marked_nodes(marker!(sink))
            .filter(|s| ctx.flows_to(commit, *s, EdgeSelection::Both))
            .peekable();

        if stores.peek().is_none() {
            continue;
        }
        any_sink_reached = true;

        let new_resources = ctx.influencees(commit, EdgeSelection::Data)
            .filter(|n| ctx.has_marker(marker!(new_resource), *n))
            .collect::<Box<[_]>>();

        // All checks that flow from the commit but not from a new_resource
        let valid_checks = ctx.influencees(commit, EdgeSelection::Data)
            .filter(|check|
                ctx.has_marker(check_rights, *check)
                && new_resources.iter().all(|r| !ctx.flows_to(*r, *check, EdgeSelection::Data))
            ).peekable();


        let mut valid_checks = ctx.marked_nodes(check_rights)
            .filter(|n| ctx.flows_to(commit, *n, EdgeSelection::Data))
            .filter(|n| ctx.any_flows(&new_resources, &[*n], EdgeSelection::Data).is_none())
            .peekable();

        if valid_checks.peek().is_none() {
            let mut err = ctx.struct_node_error(commit, "No valid checks found for this commit");
            for store in stores {
                err.with_node_warning(store, "Commit reaches this store");
            }

            for check in ctx.marked_nodes(check_rights) {
                if ctx.any_flows(&new_resources, &[check], EdgeSelection::Data).is_some() {
                    err.with_node_note(check, "This would be a valid check, but it is influenced by `new_resource`");
                } else {
                    err.with_node_note(check, "This would be a valid check but it is not influenced by the commit");
                }
            }
            err.emit();
        }

        // BELOW IS VALID POLICY CODE BUT DOESN'T WORK BC OF PARALEGAL BUG ------
        // for store in stores {
        //     // A valid check determines the store
        //     let mut check_store = valid_checks.iter().filter(|c| ctx.determines_ctrl(**c, store));
        //     assert_error!(ctx, check_store.next().is_some(), "No valid checks have control-flow influence on store {}", ctx.describe_node(store));
        // }
    }
    assert_warning!(
        ctx,
        any_sink_reached,
        "No sink was reached across controllers, the policy may be vacuous or the markers not correctly assigned/unreachable."
    );

    Ok(())
});

#[derive(Parser)]
struct Arguments {
    #[clap(long)]
    buggy: bool,
    #[clap(long)]
    skip_compile: bool,
    #[clap(long, default_value = "..")]
    directory: std::path::PathBuf,
}

fn main() -> Result<()> {
    let dir = "../";
    let args: &'static _ = Box::leak(Box::new(Arguments::parse()));
    let graph_loc = if args.skip_compile {
        GraphLocation::std(dir)
    } else {
        let mut cmd = paralegal_policy::SPDGGenCommand::global();
        cmd.external_annotations("external-annotations.toml")
            .abort_after_analysis();

        cmd.get_command()
            .args(["--target", "atomic_lib", "--", "--lib", "--features", "db"]);

        if !args.buggy {
            cmd.get_command().args(["--features", "bug-fix"]);
        }
        cmd.run(dir)?
    };

    let result = graph_loc.with_context(check_rights)?;
    println!(
        "Policy {}successful with {}",
        if result.success { "" } else { "un" },
        result.stats
    );
    Ok(())
}
