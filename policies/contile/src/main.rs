use anyhow::{Ok, Result};
use clap::Parser;
use paralegal_policy::{
    algo::ahb::TraceLevel,
    paralegal_spdg::{GlobalNode, Identifier},
    Config, Context, Diagnostics, EdgeSelection, GraphLocation, NodeExt, NodeQueries,
    SPDGGenCommand,
};
use std::{fs::File, path::PathBuf, sync::Arc};

fn policy(ctx: Arc<Context>) -> Result<()> {
    let m_sink = Identifier::new_intern("sink");
    let m_sensitive = Identifier::new_intern("sensitive");
    let m_send = Identifier::new_intern("metrics_server");
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
    )?;
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
    )?;
    Ok(())
}

#[derive(Parser)]
struct Arguments {
    #[clap(long, default_value = "case-studies/contile")]
    repo_dir: PathBuf,
    #[clap(long)]
    skip_compile: bool,
    #[clap(long)]
    dump_analyzed_code: Option<PathBuf>,
    #[clap(last = true)]
    extra_args: Vec<String>,
}

fn main() -> Result<()> {
    let args: &'static _ = Box::leak(Box::new(Arguments::parse()));

    let graph = if args.skip_compile {
        GraphLocation::std(&args.repo_dir)
    } else {
        let mut cmd = SPDGGenCommand::global();
        cmd.abort_after_analysis()
            .external_annotations("external-annotations.toml");
        cmd.get_command().args(args.extra_args.iter());
        if !args.extra_args.contains(&"--".to_owned()) {
            cmd.get_command().arg("--");
        }
        cmd.get_command().arg("--lib");
        cmd.run(&args.repo_dir)?
    };
    let mut config = Config::default();
    config.always_happens_before_tracing = TraceLevel::Full;
    let result = graph.with_context_configured(config, |ctx| {
        if let Some(path) = args.dump_analyzed_code.as_ref() {
            ctx.write_analyzed_code(File::create(path)?, false)?;
        }
        policy(ctx)
    })?;

    assert!(result.success);
    Ok(())
}
