use anyhow::Result;
use paralegal_policy::{
    assert_error, assert_warning, paralegal_spdg::Identifier, Context, Diagnostics, EdgeSelection,
    Marker, NodeExt, NodeQueries,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, sync::Arc};

#[derive(Clone, Copy, clap::ValueEnum, strum::AsRefStr)]
#[clap(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum Controllers {
    CreateApiKey,
    PaymentsAuthorizeData,
    SetupMandateRouterData,
}

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
    let sources = ctx
        .nodes_marked_any_way(marker!(apikey))
        .collect::<Box<_>>();
    assert_error!(ctx, !sources.is_empty());
    let sinks = ctx
        .nodes_marked_any_way(marker!(externalize))
        .collect::<HashSet<_>>();
    assert_error!(ctx, !sinks.is_empty());
    ctx.always_happens_before(
        sources.iter().copied(),
        |node| {
            ctx.has_marker(marker!(hashed), node) || ctx.has_marker(marker!(apikey_response), node)
        },
        |node| sinks.contains(&node),
    )?
    .report(ctx);
    Ok(())
});

policy!(card_encryption, ctx, {
    let sources = ctx
        .nodes_marked_any_way(marker!(credit_card))
        .collect::<Box<_>>();
    let sinks = ctx
        .nodes_marked_any_way(marker!(store))
        .collect::<HashSet<_>>();
    assert_error!(
        ctx,
        !sources.is_empty(),
        "VACUITY: No credit card sources found"
    );
    assert_error!(ctx, !sinks.is_empty(), "VACUITY: No sinks found");
    let encrypts = ctx.marked_nodes(marker!(encrypt)).collect::<HashSet<_>>();
    assert_warning!(ctx, encrypts.is_empty(), "WARN: No encryptors found");
    ctx.always_happens_before(
        sources.iter().copied(),
        |node| encrypts.contains(&node),
        |node| sinks.contains(&node),
    )?
    .report(ctx);
    Ok(())
});

policy!(card_storage, ctx {
    let m_store = marker!(store);
    let m_future_usage = marker!(future_usage_decision);
    let m_credit_card = marker!(credit_card);
    let mut any_sink_reached = false;
    let mut any_source_found = false;
    for ctx in ctx.controller_contexts() {
        let mut srcs = ctx
            .nodes_marked_any_way(m_credit_card)
            .filter(|n| n.controller_id() == ctx.id())
            .peekable();
        let decision_sources = ctx
            .nodes_marked_any_way(m_future_usage)
            .filter(|n| n.controller_id() == ctx.id())
            .collect::<Vec<_>>();
        any_source_found = any_source_found || srcs.peek().is_some();
        for src in srcs {
            for sink in src.influencees(&ctx, EdgeSelection::Data).into_iter().filter(|n| n.has_marker(&ctx, m_store)) {
                any_sink_reached = true;

                let decision_reaches = decision_sources.iter().cloned().any(|decision_source|
                    decision_source.has_ctrl_influence(sink, &ctx)
                );
                assert_error!(ctx, decision_reaches);
            }
        }
    }
    assert_warning!(ctx, any_source_found, "VACUITY: No sensitive sources found");
    assert_warning!(ctx, any_sink_reached, "VACUITY: No sensitive sinks ever reached.");
    ctx.note("Card storage policy finished");
    Ok(())
});

#[derive(Debug, Clone, Copy, clap::ValueEnum, Deserialize, Serialize, strum::AsRefStr)]
#[strum(serialize_all = "kebab-case")]
#[serde(rename_all = "kebab-case")]
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
