use anyhow::Result;
use paralegal_policy::{
    assert_error, assert_warning,
    paralegal_spdg::{GlobalNode, Identifier},
    Context, EdgeSelection, Marker, NodeQueries,
};
use serde::{Deserialize, Serialize};
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
    let m_future_usage = marker!(future_usage_decision);
    let m_credit_card = marker!(credit_card);
    let mut srcs = ctx.nodes_marked_any_way(m_credit_card).peekable();
    let decision_sources = ctx.nodes_marked_any_way(m_future_usage).collect::<Vec<_>>();
    assert_warning!(ctx, srcs.peek().is_some());
    let mut any_sink_reached = false;
    let sinks = ctx.nodes_marked_any_way(marker!(store)).collect::<Vec<_>>();
    assert_warning!(ctx, !sinks.is_empty());
    for src in srcs {
        for sink in sinks.iter().cloned() {
            if !src.flows_to(sink, &ctx, EdgeSelection::Data) {
                continue;
            }
            any_sink_reached = true;

            let decision_reaches = decision_sources.iter().cloned().any(|decision_source|
                decision_source.has_ctrl_influence(sink, &ctx)
            );
            assert_error!(ctx, decision_reaches);
        }
    }
    assert_warning!(ctx, any_sink_reached);
    Ok(())
});

#[derive(Debug, Clone, Copy, clap::ValueEnum, Deserialize, Serialize, strum::AsRefStr)]
#[strum(serialize_all = "kebab-case")]
pub enum Policy {
    CardStorage,
    CardEncryption,
    ApikeyStorage,
}

impl Policy {
    pub fn runnable(self) -> fn(Arc<Context>) -> Result<()> {
        use Policy::*;
        match self {
            CardStorage => card_storage as fn(Arc<Context>) -> Result<()>,
            CardEncryption => card_encryption as _,
            ApikeyStorage => apikey_storage as _,
        }
    }
}
