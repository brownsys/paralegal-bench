extern crate anyhow;
extern crate paralegal_policy;

use anyhow::Result;
use paralegal_policy::{
    assert_error, assert_warning,
    paralegal_spdg::{
        Annotation, CallSite, Ctrl, DataSink, DataSource, DefKind, Identifier, ObjectType,
    },
    Context, ControllerId, DefId, Marker,
};
use std::sync::Arc;

macro_rules! marker {
    ($id:ident) => {
        Marker::new_intern(stringify!($id))
    };
}

trait ContextExt {
    fn marked_type<'a>(&'a self, marker: Marker) -> Box<dyn Iterator<Item = DefId> + 'a>;
    fn arguments(&self, cs: &CallSite) -> Box<dyn Iterator<Item = DataSink>>;
    fn is_marked(&self, did: DefId, marker: Identifier) -> bool;
    fn any_flows<'a>(
        &self,
        ctrl_id: ControllerId,
        from: &[&'a DataSource],
        to: &[&'a DataSink],
    ) -> Option<(&'a DataSource, &'a DataSink)>;
}

impl ContextExt for Context {
    fn marked_type<'a>(&'a self, marker: Marker) -> Box<dyn Iterator<Item = DefId> + 'a> {
        Box::new(
            self.marked(marker)
                .filter(|(did, _)| self.desc().def_info[did].kind == DefKind::Type)
                .map(|(did, refinement)| {
                    assert!(refinement.on_self());
                    *did
                }),
        ) as Box<_>
    }

    fn arguments(&self, cs: &CallSite) -> Box<dyn Iterator<Item = DataSink>> {
        let (_, ObjectType::Function(args)) = self.desc().annotations[&cs.function] else {
            return Box::new([].into_iter()) as Box<_>;
        };

        let cs = cs.clone();

        Box::new((0..args).map(move |i| DataSink::Argument {
            function: cs.clone(),
            arg_slot: i,
        })) as Box<_>
    }

    fn is_marked(&self, did: DefId, marker: Identifier) -> bool {
        self.desc().annotations[&did]
            .0
            .iter()
            .any(|m| matches!(m, Annotation::Marker(m) if m.marker == marker))
    }

    fn any_flows<'a>(
        &self,
        ctrl_id: ControllerId,
        from: &[&'a DataSource],
        to: &[&'a DataSink],
    ) -> Option<(&'a DataSource, &'a DataSink)> {
        from.iter().find_map(|&src| {
            to.iter()
                .find_map(|&sink| self.flows_to(ctrl_id, src, sink).then_some((src, sink)))
        })
    }
}

trait CtrlExt {
    fn data_sources<'a>(&'a self) -> Box<dyn Iterator<Item = &'a DataSource> + 'a>;
}

impl CtrlExt for Ctrl {
    fn data_sources<'a>(&'a self) -> Box<dyn Iterator<Item = &'a DataSource> + 'a> {
        Box::new(self.data_flow.keys())
    }
}

fn check(ctx: Arc<Context>) -> Result<()> {
    let pageview_data = ctx.marked_type(marker!(pageviews)).collect::<Vec<_>>();
    assert_warning!(
        ctx,
        !pageview_data.is_empty(),
        "No pageview data found. The policy may be vacuous."
    );
    ctx.named_policy("expiration", |ctx| {
        let found = ctx.clone().controller_contexts().any(|ctx| {
            let delete_sinks = ctx
                .marked_sinks(ctx.current().data_sinks(), marker!(to_delete))
                .collect::<Vec<_>>();
            let time_marker = marker!(time);
            let time_source = ctx.current().data_sources().filter(|ds| matches!(ds, DataSource::FunctionCall(cs) if ctx.is_marked(cs.function, time_marker))).collect::<Vec<_>>();
            pageview_data.iter().all(|typ| {
                let sources = ctx.srcs_with_type(ctx.current(), *typ).collect::<Vec<_>>();
                ctx.current().data_flow.keys().filter_map(DataSource::as_function_call).any(|cs| {
                    let cs_as_source = DataSource::FunctionCall(cs.clone());
                    ctx.arguments(cs).any(|arg| ctx.any_flows(ctx.id(), &sources, &[&arg]).is_some())
                    && ctx.arguments(cs).any(|arg| ctx.any_flows(ctx.id(), &time_source, &[&arg]).is_some())
                    && ctx.any_flows(ctx.id(), &[&cs_as_source], &delete_sinks).is_some()
                })
            })
        });
        assert_error!(ctx, !found, "Could not find an expiration deletion.")
    });
    Ok(())
}

fn main() -> Result<()> {
    // The directory where the project-to-analyze is
    let dir = "..";
    paralegal_policy::SPDGGenCommand::global()
        .run(dir)?
        .with_context(check)
}
