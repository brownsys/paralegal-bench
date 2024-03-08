extern crate anyhow;
extern crate clap;
extern crate paralegal_policy;

use anyhow::Result;
use paralegal_policy::{
    assert_error, assert_warning,
    paralegal_spdg::{GlobalNode, Identifier, Node},
    Context, Diagnostics, EdgeSelection, Marker,
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
    )?
    .report(ctx);
    Ok(())
});

policy!(card_storage, ctx {
    let mut srcs = ctx.marked_nodes(marker!(credit_card)).peekable();
    let decision_sources = ctx.marked_nodes(marker!(future_usage_decision)).collect::<Vec<_>>();
    assert_warning!(ctx, srcs.peek().is_some());
    let mut any_sink_reached = false;
    let sinks = ctx.marked_nodes(marker!(store)).collect::<Vec<_>>();
    assert_warning!(ctx, !sinks.is_empty());
    for src in srcs {
        for sink in sinks.iter().cloned() {
            if !ctx.flows_to(src, sink, EdgeSelection::Data) {
                continue;
            }
            any_sink_reached = true;

            let decision_reaches = decision_sources.iter().cloned().any(|decision_source|
                ctx.determines_ctrl(decision_source, sink)
            );
            assert_error!(ctx, decision_reaches);
        }
    }
    assert_warning!(ctx, any_sink_reached);
    Ok(())
});

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum Policy {
    CardStorage,
    CardEncryption,
    ApikeyStorage,
}

impl Policy {
    fn runnable(self) -> fn(Arc<Context>) -> Result<()> {
        use Policy::*;
        match self {
            CardStorage => card_storage as fn(Arc<Context>) -> Result<()>,
            CardEncryption => card_encryption as _,
            ApikeyStorage => apikey_storage as _,
        }
    }
}

#[derive(clap::Parser)]
struct Args {
    #[clap(long, short)]
    policy: Option<Vec<Policy>>,
    #[clap(long, default_value = "..")]
    source_dir: std::path::PathBuf,
}

fn main() -> Result<()> {
    let mut cmd = paralegal_policy::SPDGGenCommand::global();
    cmd.abort_after_analysis();
    cmd.external_annotations("external-annotations.toml");
    cmd.get_command()
        .args(["--target", "router", "--", "--lib"]);
    use clap::{Parser, ValueEnum};
    let args: &'static _ = Box::leak(Box::new(Args::parse()));
    cmd.run(&args.source_dir)?.with_context(|ctx| {
        for p in args
            .policy
            .as_ref()
            .map_or(Policy::value_variants(), Vec::as_slice)
        {
            p.runnable()(ctx.clone())?
        }
        Ok(())
    })?;
    println!("Policy successful");
    Ok(())
}
