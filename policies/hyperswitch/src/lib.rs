use anyhow::Result;
use paralegal_policy::{
    assert_error, assert_warning,
    paralegal_spdg::{DisplayPath, Identifier, NodeCluster},
    Context, Diagnostics, EdgeSelection, IntoIterGlobalNodes, Marker, NodeExt, NodeQueries,
    RootContext,
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
        fn $name(ctx: Arc<RootContext>) -> Result<()> {
            ctx.named_policy(Identifier::new_intern(stringify!($name)), |$context| $code)
        }
    };
}

pub mod cnl {
    pub mod card_storage {
        include!(concat!(env!("OUT_DIR"), "/card-storage.rs"));
    }

    pub mod card_encryption {
        include!(concat!(env!("OUT_DIR"), "/card-encryption.rs"));
    }

    pub mod apikey_storage {
        include!(concat!(env!("OUT_DIR"), "/apikey-storage.rs"));
    }
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
        if srcs.peek().is_none() {
            continue;
        }
        let Some(decision_sources) = NodeCluster::try_from_iter(
            ctx.nodes_marked_any_way(m_future_usage)
                .filter(|n| n.controller_id() == ctx.id())
            ) else {
            let mut msg = ctx.struct_error("No future usage decision found");
            for src in srcs {
                msg.with_node_note(src, "Credit card source");
            }
            msg.emit();
            continue;
        };
        any_source_found = any_source_found || srcs.peek().is_some();
        // for src in srcs {
        //     for sink in src.influencees(&ctx, EdgeSelection::Data).into_iter().filter(|n| n.has_marker(&ctx, m_store)) {
        //         any_sink_reached = true;

        //         if let Some(_) = decision_sources.has_ctrl_influence_all(sink, &ctx) {
        //             let mut msg = ctx.struct_error("Unprotected credit card storage");
        //             msg.with_node_note(src, "Credit card source");
        //             msg.with_node_note(sink, "Credit card storage");
        //             msg.emit();
        //         }
        //     }
        // }

        // Optimization 1: Ask control influence on all sinks for one credit
        // card value at the same time.
        for src in srcs {
            let Some(sinks) = NodeCluster::try_from_iter(src.influencees(&ctx, EdgeSelection::Data).into_iter().filter(|n| n.has_marker(&ctx, m_store))) else {
                continue;
            };
            any_sink_reached = true;

            if let Some(unreached) = decision_sources.has_ctrl_influence_all(&sinks, &ctx) {
                let mut msg = ctx.struct_error("Unprotected credit card storage");
                msg.with_node_note(src, "Credit card source");
                for sink in unreached.iter_global_nodes() {
                    msg.with_node_note(sink, "Credit card storage");
                }
                msg.emit();
            }
        }

        // Optimization 2: Ask control influence on all sinks for all credit
        // card values at the same time.
        // if let Some(sinks) = NodeCluster::try_from_iter(srcs.
        //     flat_map(|src| src.influencees(&ctx, EdgeSelection::Data))
        //     .filter(|n| n.has_marker(&ctx, m_store)))
        // {
        //     any_sink_reached = true;
        //     if let Some(unreached) = decision_sources.has_ctrl_influence_all(&sinks, &ctx) {
        //         let mut msg = ctx.struct_error("Unprotected credit card storage");
        //         for sink in unreached.iter_global_nodes() {
        //             msg.with_node_note(sink, "Credit card storage");
        //         }
        //         msg.emit();
        //     }
        // }

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
    pub fn runnable(self, cnl: bool) -> fn(Arc<RootContext>) -> Result<()> {
        use Policy::*;
        if cnl {
            match self {
                CardStorage => cnl::card_storage::check as fn(Arc<RootContext>) -> Result<()>,
                CardEncryption => cnl::card_encryption::check as _,
                ApikeyStorage => cnl::apikey_storage::check as _,
            }
        } else {
            match self {
                CardStorage => card_storage as fn(Arc<RootContext>) -> Result<()>,
                CardEncryption => card_encryption as _,
                ApikeyStorage => apikey_storage as _,
            }
        }
    }
}
