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
use std::{ops::Deref, sync::Arc};

macro_rules! marker {
    ($id:ident) => {
        Marker::new_intern(stringify!($id))
    };
}

/// Monadic quantifiers for Rust iterators.
///
/// Allows you to pseudo-monadically create a `bool` computation with iterators.
/// It exposes the iterator methods `any` and `all` as prefix bindings and also
/// enables pattern matching and guarding.
///
/// The macro expands a sequence of statements. All usual Rust statements are
/// supported and only the top-level statements are expanded with the special
/// syntax, not e.g. nested blocks.
///
/// It supports the following syntax:
///
/// ```ignored
/// any pattern <- source;
/// all pattern <- source;
/// guard expression;
/// ```
///
/// `any` and `all` correspond to the usual iterator methods, but the difference
/// is that they do not require nesting. Instead the statements following them
/// are interpreted as their body. In addition the `pattern` failure is handled
/// implicitly. In the case of `all`, if the pattern doesn't match it simply
/// returns `true`, e.g. it only enforces the subsequent conditions for matched
/// patterns. In the case of `any` a failing pattern match returns `false`, e.g.
/// the search for a matching element continues.
///
/// `guard condition` enforces `condition`. If the condition does not hold
/// `false` is returned. You may also use it as `guard pattern = expr` in which
/// case `false` is returned if the pattern does not match.
macro_rules! iterator_quantifiers {
    (require $e:expr; $($rest:tt)*) => {
        if !$e {
            return false;
        }
        iterator_quantifiers!($($rest)*);
    };
    (require $pat:pat = $e:expr; $($rest:tt)*) => {
        let $pat = !$e {
            return false
        };
        iterator_quantifiers!($($rest)*);
    };
    (allow $e:expr; $($rest:tt)*) => {
        if $e {
            return true;
        }
        iterator_quantifiers!($($rest)*);
    };
    (any $pat:pat_param = $e:expr; $($rest:tt)*) => {
        return $e.into_iter().any(|ident| if let $pat = ident {
                iterator_quantifiers!($($rest)*);
        } else {
            false
        });
    };
    (all $pat:pat_param = $e: expr; $($rest:tt)*) => {
        return $e.into_iter().all(|ident| if let $pat = ident {
            iterator_quantifiers!($($rest)*);
        } else { true })
    };
    ($s:stmt; $($rest:tt)*) => {
        $s;
        iterator_quantifiers!($($rest)*)
    };
    ($e:expr) => {
        return $e
    };
}

trait Markeable {
    fn is_marked(&self, ctx: &Context, marker: Identifier) -> bool;
}

impl Markeable for DefId {
    fn is_marked(&self, ctx: &Context, marker: Identifier) -> bool {
        ctx.annotations_for(*self)
            .iter()
            .any(|m| matches!(m, Annotation::Marker(m) if m.marker == marker))
    }
}

impl Markeable for CallSite {
    fn is_marked(&self, ctx: &Context, marker: Identifier) -> bool {
        iterator_quantifiers!(
            any Annotation::Marker(m) = &ctx.annotations_for(self.function);
            m.marker == marker
                && m.refinement.on_return()
        )
    }
}

struct DisplayDef<'a> {
    def_id: DefId,
    ctx: &'a Context,
}

impl<'a> std::fmt::Display for DisplayDef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::fmt::Write;
        let info = &self.ctx.desc().def_info[&self.def_id];
        f.write_str(match info.kind {
            DefKind::Type => "type",
            DefKind::Function => "function",
        })?;
        f.write_str(" `")?;
        for segment in &info.path {
            f.write_str(segment.as_str())?;
            f.write_str("::")?;
        }
        f.write_str(info.name.as_str())?;
        f.write_char('`')
    }
}

trait ContextExt {
    fn marked_type<'a>(&'a self, marker: Marker) -> Box<dyn Iterator<Item = DefId> + 'a>;
    fn arguments<'a>(&'a self, cs: &'a CallSite) -> Box<dyn Iterator<Item = &'a DataSink> + 'a>;
    fn annotations_for(&self, id: DefId) -> &[Annotation];
    fn describe_def(&self, def_id: DefId) -> DisplayDef;
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

    fn arguments<'a>(&'a self, cs: &'a CallSite) -> Box<dyn Iterator<Item = &'a DataSink> + 'a> {
        Box::new(self.desc().all_sinks()
            .into_iter()
            .filter(move |snk| matches!(snk, DataSink::Argument { function, .. } if function == cs)))
    }

    fn annotations_for(&self, id: DefId) -> &[Annotation] {
        self.desc()
            .annotations
            .get(&id)
            .map_or(&[] as &[_], |it| it.0.as_slice())
    }
    fn describe_def(&self, def_id: DefId) -> DisplayDef {
        DisplayDef { ctx: self, def_id }
    }
}

trait CtrlExt {
    fn data_sources<'a>(&'a self) -> Box<dyn Iterator<Item = &'a DataSource> + 'a>;
    fn call_sites_for<'a>(&'a self, fun: DefId) -> Box<dyn Iterator<Item = &CallSite> + 'a>;
}

impl CtrlExt for Ctrl {
    fn data_sources<'a>(&'a self) -> Box<dyn Iterator<Item = &'a DataSource> + 'a> {
        Box::new(self.data_flow.keys())
    }

    fn call_sites_for<'a>(&'a self, fun: DefId) -> Box<dyn Iterator<Item = &CallSite> + 'a> {
        Box::new(self.data_sources().filter_map(move |ds| match ds {
            DataSource::FunctionCall(f) if f.function == fun => Some(f),
            _ => None,
        }))
    }
}

fn check(ctx: Arc<Context>) -> Result<()> {
    let pageview_data = ctx.marked(marker!(pageviews)).map(|d| d.0).collect::<Vec<_>>();
    ctx.clone().named_policy("date store", |ctx| {
        assert_warning!(
            ctx,
            !pageview_data.is_empty(),
            "No pageview data found. The policy may be vacuous."
        );
        let farthest = std::sync::atomic::AtomicI64::default();

        let tick = |num| {
            farthest.fetch_max(num, std::sync::atomic::Ordering::Relaxed);
        };
        let mut found_store_controller = false;
        ctx.controller_contexts().all(|ctx|{
            println!("Checking {}", ctx.describe_def(ctx.id()));
            let db_store_marker = marker!(db_store);
            ctx.report_marker_if_absent(db_store_marker);
            let time_marker = marker!(time);
            ctx.report_marker_if_absent(time_marker);
            let time_sources = ctx
                .current()
                .data_sources()
                .filter(|ds|
                    matches!(ds, DataSource::FunctionCall(cs)
                                    if cs.function.is_marked(ctx.deref(), time_marker))
                ).collect::<Vec<_>>();
            iterator_quantifiers!(
                all typ = pageview_data.iter();
                all type_ident_call_site = ctx.current().call_sites_for(*typ);
                tick(1);
                let type_ident = DataSource::FunctionCall(type_ident_call_site.clone());
                all sink = ctx.marked_sinks(ctx.current().data_sinks(), db_store_marker);
                tick(2);
                let type_is_stored = ctx.flows_to(ctx.id(), &type_ident, &sink.clone().into());
                allow !type_is_stored;
                tick(3);
                found_store_controller = true;
                assert_error!(ctx, !time_sources.is_empty(), "Found store but no local source of time");
                any time_source = &time_sources;
                assert_error!(ctx, ctx.flows_to(ctx.id(), time_source, &sink.clone().into()), "Found store and local source of time, but no connection");
                true
            )
        });
        assert_warning!(ctx, found_store_controller, "No controller storing pageviews found, policy may be vacuous");
        println!("Last seen for first policy {}", farthest.load(std::sync::atomic::Ordering::Relaxed));
    });
    ctx.named_policy("expiration check", |ctx| {
        assert_warning!(
            ctx,
            !pageview_data.is_empty(),
            "No pageview data found. The policy may be vacuous."
        );
        let farthest = std::sync::atomic::AtomicI64::default();

        let tick = |num| {
            farthest.fetch_max(num, std::sync::atomic::Ordering::Relaxed);
        };

        let db_access_marker = marker!(db_access);
        let found = ctx.controller_contexts().any(|ctx| {
            let delete_sinks = ctx
                .marked_sinks(ctx.current().data_sinks(), marker!(deletes))
                .collect::<Vec<_>>();
            let time_marker = marker!(time);
            let time_sources = ctx
                .current()
                .data_sources()
                .filter(|ds|
                    matches!(ds, DataSource::FunctionCall(cs)
                                    if cs.function.is_marked(ctx.deref(), time_marker))
                ).collect::<Vec<_>>();
            iterator_quantifiers!(
                all typ = pageview_data.iter();
                any time_source = &time_sources;
                tick(1);
                any type_ident_call_site = ctx.current().call_sites_for(*typ);
                let type_ident = DataSource::FunctionCall(type_ident_call_site.clone());
                tick(2);
                any type_source@DataSource::FunctionCall(f) = ctx.current().data_sources();
                tick(3);
                require f.is_marked(ctx.deref(), db_access_marker);
                tick(4);
                require ctx.arguments(f).any(|arg| ctx.flows_to(ctx.id(), &type_ident, &arg.clone().into()));
                tick(5);
                any delete@DataSink::Argument { function: delete_call_site, .. } = &delete_sinks;
                let ref delete = (*delete).clone().into();
                tick(6);
                require ctx.flows_to(ctx.id(), type_source, delete);
                tick(6);
                any time_check = ctx.current().ctrl_flow.keys().filter_map(DataSource::as_function_call);
                tick(7);
                let arguments = ctx.arguments(time_check).collect::<Vec<_>>();
                any time_check_time_arg = &arguments;
                any time_check_type_arg = &arguments;
                let type_flows_to_check = ctx.flows_to(ctx.id(), &type_source, &(*time_check_type_arg).clone().into());
                tick(8);
                require type_flows_to_check;
                let time_flows_to_check = ctx.flows_to(ctx.id(), &time_source, &(*time_check_time_arg).clone().into());
                tick(9);
                require time_flows_to_check;
                let check_ctrls_delete = ctx.current().ctrl_flow[&DataSource::FunctionCall(time_check.clone())].contains(delete_call_site);
                tick(10);
                check_ctrls_delete
            )
        });
        println!("Last seen {}", farthest.load(std::sync::atomic::Ordering::Relaxed));
        assert_error!(ctx, found, "Could not find an expiration deletion for pageview data.")
    });
    Ok(())
}

fn main() -> Result<()> {
    let dir = "..";
    let mut cmd = paralegal_policy::SPDGGenCommand::global();
    cmd.external_annotations("external-annotations.toml")
        .abort_after_analysis()
        .get_command()
        .args([
        "--eager-local-markers",
        "--inline-elision",
        "--",
        "--lib",
    ]);
    cmd.run(dir)?.with_context(check)?;
    println!("Policy check succeeded");
    Ok(())
}
