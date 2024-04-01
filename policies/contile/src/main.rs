use anyhow::{Ok, Result};
use clap::Parser;
use paralegal_policy::{
    algo::ahb::TraceLevel,
    paralegal_spdg::{GlobalNode, Identifier},
    Config, Context, Diagnostics, EdgeSelection, GraphLocation, NodeQueries, SPDGGenCommand,
};
use std::{fs::File, path::PathBuf, sync::Arc};

fn policy(ctx: Arc<Context>) -> Result<()> {
    let m_sink = Identifier::new_intern("sink");
    let m_sensitive = Identifier::new_intern("sensitive");
    let m_send = Identifier::new_intern("metrics_server");
    ctx.clone().named_policy(
        Identifier::new_intern("personal tags not in metrics"),
        |ctx| {
            for sink in ctx.nodes_marked_any_way(m_sink) {
                for src in ctx.nodes_marked_any_way(m_sensitive) {
                    let mut intersections = sink
                        .influencers(&ctx, EdgeSelection::Data)
                        .into_iter()
                        .filter(|intersection| {
                            src.flows_to(*intersection, &ctx, EdgeSelection::Data)
                        });
                    if let Some(intersection) = intersections.next() {
                        let mut msg = ctx
                            .struct_node_error(intersection, "This call releases sensitive data");
                        msg.with_node_note(src, "Sensitive data originates here");
                        msg.with_node_note(intersection, "Externalizing value originates here");
                        msg.emit();
                    }
                }
            }
            Ok(())
        },
    )?;
    ctx.named_policy(Identifier::new_intern("personal tags not sent"), |ctx| {
        let personals = ctx.nodes_marked_any_way(m_sensitive).collect::<Box<[_]>>();
        let sends = ctx.nodes_marked_any_way(m_send).collect::<Box<[_]>>();
        if let Some((from, to)) = ctx.any_flows(&personals, &sends, EdgeSelection::Data) {
            ctx.always_happens_before([from], |_| false, |t| t == to)?
                .report(ctx);
            // let mut msg = ctx.struct_node_error(to, "This call externalizes a sensitive value");
            // msg.with_node_note(from, "Sensitive data originates here");
            // msg.emit();
        }
        Ok(())
    })
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
            ctx.write_analyzed_code(File::create(path)?, true)?;
        }
        policy(ctx)
    })?;

    assert!(result.success);
    Ok(())
}
