extern crate anyhow;
extern crate paralegal_policy;

use anyhow::{bail, Result};
use paralegal_policy::{
    assert_error, assert_warning, paralegal_spdg::Identifier, Context, Diagnostics, EdgeType,
    Marker, Node,
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
    fn marked_nodes<'a>(&'a self, marker: Marker) -> Box<dyn Iterator<Item = Node<'a>> + 'a>;

    fn determines_ctrl(&self, influencer: Node, target: Node) -> bool;
}

impl ContextExt for Context {
    fn marked_nodes<'a>(&'a self, marker: Marker) -> Box<dyn Iterator<Item = Node<'a>> + 'a> {
        Box::new(
            self.desc()
                .controllers
                .keys()
                .copied()
                .flat_map(move |k| self.all_nodes_for_ctrl(k))
                .filter(move |node| self.has_marker(marker, *node)),
        )
    }

    fn determines_ctrl(&self, influencer: Node, target: Node) -> bool {
        let Some(tcs) = target.associated_call_site() else {
            self.error(format!("{target:?} cannot be influenced by control flow"));
            return false;
        };

        self.influencees(influencer, EdgeType::Data)
            .any(|inf| self.flows_to(inf, tcs, EdgeType::Control))
    }
}

policy!(check_rights, ctx {
    let commits = ctx.marked_nodes(marker!(commit));
    let mut any_sink_reached = false;
    for commit in commits {
        // If commit is stored
        let mut stores = ctx.influencees(commit, EdgeType::DataAndControl)
            .filter(|s| ctx.has_marker(marker!(sink), *s))
            .peekable();

        if stores.peek().is_none() {
            continue;
        }
        any_sink_reached = true;

        let new_resources = ctx.influencees(commit, EdgeType::Data)
            .filter(|n| ctx.has_marker(marker!(new_resource), *n))
            .collect::<Vec<_>>();

        // All checks that flow from the commit but not from a new_resource
        let valid_checks = ctx.influencees(commit, EdgeType::Data)
            .filter(|check|
                ctx.has_marker(marker!(check_rights), *check)
                && new_resources.iter().all(|r| !ctx.flows_to(*r, *check, EdgeType::Data))
            )
            .collect::<Vec<_>>();
        assert_error!(
            ctx,
            !valid_checks.is_empty(),
            format!(
                "Found no valid checks for commit {} which flows into {}",
                ctx.describe_node(commit),
                ctx.describe_node(*stores.peek().unwrap())
            )
        );

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

fn main() -> Result<()> {
    let dir = "../";
    let buggy_arg = std::env::args().nth(1);
    let mut cmd = paralegal_policy::SPDGGenCommand::global();

    cmd.get_command().args([
        "--inline-elision",
        "--model-version",
        "v2",
        "--skip-sigs",
        "--external-annotations",
        "external-annotations.toml",
        "--target",
        "atomic_lib",
        "--abort-after-analysis",
        "--",
        "-p",
        "atomic_lib",
        "--lib",
        "--features",
        "db",
    ]);

    match buggy_arg {
        Some(s) => match s.as_str() {
            "buggy" => (),
            "fixed" => {
                cmd.get_command().args(["--features", "bug-fix"]);
            }
            other => bail!("don't recognize the flag '{other}'"),
        },
        None => {
            cmd.get_command().args(["--features", "bug-fix"]);
        }
    };

    cmd.run(dir)?.with_context(check_rights)?;
    println!("Policy successful");
    Ok(())
}
