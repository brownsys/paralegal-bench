extern crate anyhow;
extern crate paralegal_policy;

use anyhow::Result;
use paralegal_policy::{assert_error, paralegal_spdg::Identifier, Context, Marker, Node};
use std::sync::Arc;

macro_rules! marker {
    ($name:ident) => {{
        lazy_static::lazy_static! {
            static ref MARKER: Marker = Identifier::new_intern(stringify!($name));
        }
        *MARKER
    }};
}

trait ContextExt {
    fn marked_nodes<'a>(&'a self, marker: Marker) -> Box<dyn Iterator<Item = Node<'a>> + 'a>;
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
}

fn dummy_policy(ctx: Arc<Context>) -> Result<()> {
    ctx.always_happens_before(
        ctx.marked_nodes(marker!(apikey)),
        |node| {
            ctx.has_marker(marker!(hashed), node) || ctx.has_marker(marker!(apikey_response), node)
        },
        |node| ctx.has_marker(marker!(externalize), node),
    )?
    .report(ctx);
    Ok(())
}

fn main() -> Result<()> {
    let dir = "..";
    let mut cmd = paralegal_policy::SPDGGenCommand::global();
    cmd.get_command().args([
        "--inline-elision",
        "--external-annotations",
        "external-annotations.toml",
        "--target",
        "router",
        "--abort-after-analysis",
        "--",
        "--lib",
        "-p",
        "router",
    ]);
    cmd.run(dir)?.with_context(dummy_policy)?;
    println!("Policy successful");
    Ok(())
}
