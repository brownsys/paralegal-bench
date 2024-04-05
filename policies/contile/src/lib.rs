use anyhow::{Ok, Result};
use paralegal_policy::{paralegal_spdg::Identifier, Context, Diagnostics, EdgeSelection, NodeExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Copy, Clone, clap::ValueEnum, strum::AsRefStr, Serialize, Deserialize)]
#[strum(serialize_all = "kebab-case")]
#[serde(rename_all = "kebab-case")]
pub enum Policy {
    SendToAdm,
    SendToMetrics,
}

impl Policy {
    pub fn runnable(self) -> fn(Arc<Context>) -> Result<()> {
        match self {
            Policy::SendToAdm => send_to_adm as fn(_) -> _,
            Policy::SendToMetrics => send_to_metrics as _,
        }
    }
}

pub fn send_to_adm(ctx: Arc<Context>) -> Result<()> {
    let m_sink = Identifier::new_intern("sink");
    let m_sensitive = Identifier::new_intern("sensitive");
    ctx.clone().named_policy(
        Identifier::new_intern("personal tags not sent to adm"),
        |ctx| {
            for sink in ctx.nodes_marked_any_way(m_sink) {
                for src in ctx.nodes_marked_any_way(m_sensitive) {
                    if let Some(path) = src.shortest_path(sink, &ctx, EdgeSelection::Data) {
                        let mut msg =
                            ctx.struct_node_error(sink, "this call sends personal data to the adm");
                        msg.with_node_help(src, "personal data originates here");
                        for n in path.iter() {
                            msg.with_node_note(
                                *n,
                                format!("Passes through this {}", n.info(&ctx).description),
                            );
                        }
                        msg.emit();
                    }
                }
            }

            Ok(())
        },
    )
}

pub fn send_to_metrics(ctx: Arc<Context>) -> Result<()> {
    let m_sensitive = Identifier::new_intern("sensitive");
    let m_send = Identifier::new_intern("metrics_server");
    ctx.named_policy(
        Identifier::new_intern("personal tags not sent to metrics"),
        |ctx| {
            let personals = ctx.nodes_marked_any_way(m_sensitive).collect::<Box<[_]>>();
            let sends = ctx.nodes_marked_any_way(m_send).collect::<Box<[_]>>();
            for personal in personals.iter() {
                for send in sends.iter() {
                    if let Some(path) = personal.shortest_path(*send, &ctx, EdgeSelection::Data) {
                        let mut msg = ctx.struct_node_error(
                            *send,
                            "this call sends personal data to the metrics server",
                        );
                        msg.with_node_note(*personal, "personal data originates here");
                        for p in path.iter() {
                            msg.with_node_note(
                                *p,
                                format!("Passes through this {}", p.info(&ctx).description),
                            );
                        }
                        msg.emit();
                    }
                }
            }
            Ok(())
        },
    )
}
