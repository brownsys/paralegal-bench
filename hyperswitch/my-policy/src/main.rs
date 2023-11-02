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

macro_rules! policy {
    ($name: ident, $context: ident, $code:block) => {
        fn $name(ctx: Arc<Context>) -> Result<()> {
            ctx.named_policy(stringify!($name), |$context| $code)
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
        let Some(tcs) = target.as_data_sink(target) else {
            self.error(format!("{target} cannot be influenced by control flow"));
        };

        ctx.influencees(influencer)
            .any(|inf| ctx.flows_to(inf, tcx, EdgeType::Control))
    }
}

policy!(apikey_storage, ctx, {
    ctx.always_happens_before(
        ctx.marked_nodes(marker!(apikey)),
        |node| {
            ctx.has_marker(marker!(hashed), node) || ctx.has_marker(marker!(apikey_response), node)
        },
        |node| ctx.has_marker(marker!(externalize), node),
    )?
    .report(ctx);
    Ok(())
});

policy!(card_encryption, ctx, {
    ctx.always_happens_before(
        ctx.marked_nodes(marker!(credit_card)),
        |node| ctx.has_marker(marker!(encrypt), node),
        |node| ctx.has_marker(marker!(store), node),
    )
});

policy!(card_storage, ctx {
    let srcs = ctx.marked_nodes(marker!(credit_card)).peekable();
    let decision_sources = ctx.marked_nodes(marker!(future_usage_decision)).collect::<Vec<_>>();
    assert_warning!(ctx, srcs.peek().is_some());
    let mut any_sink_reached = false;
    for src in srcs {
        let sinks = ctx.marked_nodes(marker!(store)).peekable();
        assert_warning!(ctx, sinks.peek().is_some());
        for sink in sinks {
            if !ctx.flows_to(src, sink) {
                continue;
            }
            any_sink_reached = true;

            let decision_reaches = decision_sources.iter().any(|decision_source|
                ctx.determines_ctrl(decision_source, sink)
            );
            assert_error!(ctx, decision_reaches);
        }
    }
    assert_warning!(ctx, any_sink_reached);
    Ok(())
});

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
    cmd.run(dir)?.with_context(|ctx| {
        //apikey_storage(ctx.clone())?;
        //card_encryption(ctx.clone())?;
        card_storage(ctx.clone())?;
        Ok(())
    })?;
    println!("Policy successful");
    Ok(())
}
