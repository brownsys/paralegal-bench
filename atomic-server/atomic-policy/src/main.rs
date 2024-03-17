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
    let results = commits.filter_map(|commit| {
        let check_rights = marker!(check_rights);
        // If commit is stored
        let stores = ctx.influencees(commit, EdgeSelection::Both)
            .filter(|s| ctx.has_marker(marker!(sink), *s))
            .collect::<Box<[_]>>();
        if stores.is_empty() {
            return None;
        }
        any_sink_reached = true;

        let new_resources = ctx.influencees(commit, EdgeSelection::Data)
            .filter(|n| ctx.has_marker(marker!(new_resource), *n))
            .collect::<Box<[_]>>();

        // All checks that flow from the commit but not from a new_resource
        let valid_checks = ctx.influencees(commit, EdgeSelection::Data)
            .filter(|check|
                ctx.has_marker(check_rights, *check)
                && new_resources.iter().all(|r| !ctx.flows_to(*r, *check, EdgeSelection::Data)))
            .collect::<Box<[_]>>();

        Some(stores.iter().copied().map(|store| {
            (store, valid_checks.iter().copied().find(|check| ctx.successors(store).any(|cs| ctx.has_ctrl_influence(*check, cs))))
        }).collect::<Box<[_]>>())
    });

    let likely_result = results.max_by_key(|checks| checks.iter().filter(|(_, v)| v.is_some()).count());

    if let Some(checks) = likely_result {
        for (store, check) in checks.iter().copied() {
            if let Some(check) = check {
                let mut msg = ctx.struct_node_note(store, "This store is properly checked");
                msg.with_node_note(check, "With this check");
            } else {
                ctx.node_error(store, "This store is not protected");
            }
        }
    } else {
        ctx.error("No results at all. No controllers?")
    }
    assert_error!(
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
