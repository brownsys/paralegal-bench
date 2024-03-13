extern crate anyhow;
extern crate paralegal_policy;

use anyhow::Result;
use paralegal_policy::{
    assert_error, assert_warning,
    paralegal_spdg::{
        DefKind, Identifier, Node, GlobalNode
    },
    Context, ControllerId, DefId, Marker,
};
use std::{ops::Deref, sync::Arc};

macro_rules! marker {
    ($id:ident) => {
        {
            lazy_static::lazy_static! {
                static ref MARKER: Marker = Marker::new_intern(stringify!($id));
            }
            *MARKER
        }
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


trait ContextExt {
    fn marked_type<'a>(&'a self, marker: Marker) -> Box<dyn Iterator<Item = DefId> + 'a>;
    fn arguments<'a>(&'a self, cs: &'a CallSite) -> Box<dyn Iterator<Item GlobalNode> + 'a>;
    fn annotations_for(&self, id: DefId) -> &[Annotation];
    fn marked_sources<'a>(
        &'a self,
        ctrl_id: ControllerId,
        marker: Marker,
    ) -> Box<dyn Iterator<Item = GlobalNode> + 'a>;
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

    fn arguments<'a>(&'a self, cs: GlobalNode) -> Box<dyn Iterator<Item = GlboalNode> + 'a> {
        Box::new(self.desc().all_sinks().into_iter().filter(
            move |snk| matches!(snk, DataSink::Argument { function, .. } if function == cs),
        ))
    }

    fn annotations_for(&self, id: DefId) -> &[Annotation] {
        self.desc()
            .annotations
            .get(&id)
            .map_or(&[] as &[_], |it| it.0.as_slice())
    }

    fn marked_sources<'a>(
        &'a self,
        ctrl_id: ControllerId,
        marker: Marker,
    ) -> Box<dyn Iterator<Item = &'a DataSource> + 'a> {
        Box::new(
            self.desc().controllers[&ctrl_id]
                .data_sources()
                .filter(move |s| { s.has_marker(marker)
                }),
        )
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

/// Not actually used, because it turns out the application doesn't do this. It just cleans up the database every 10min.
fn check_no_expired_read(ctx: Arc<Context>) -> Result<()> {
    ctx.named_policy("no expired read", |ctx| {
        let expirable_data = ctx.marked(marker!(pageviews)).map(|i| i.0).collect::<Vec<_>>();
        let time_marker = marker!(time);
        ctx.report_marker_if_absent(time_marker);
        let db_access_marker = marker!(db_access);
        ctx.report_marker_if_absent(db_access_marker);
        let externalizes_marker = marker!(externalizes);
        ctx.report_marker_if_absent(externalizes_marker);
        iterator_quantifiers!(
            all ctx = ctx.controller_contexts();
            let time_sources = ctx
                .current()
                .data_sources()
                .filter(|ds| ds.has_marker(time_marker)
                ).collect::<Vec<_>>();
            all typ = &expirable_data;
            // Another option is to say expiration must be for all data,
            // with exceptions for certain marked types.
            all type_ident_call_site = ctx.current().call_sites_for(*typ);
            let type_ident = DataSource::FunctionCall(type_ident_call_site.clone());
            all type_source = ctx.marked_sources(ctx.id(), db_access_marker)
                .filter(|s|
                    matches!(s,
                        DataSource::FunctionCall(f)
                        if ctx.arguments(f)
                            .any(|arg|
                                ctx.flows_to(ctx.id(), &type_ident, &arg.clone().into()))
                    )
                );
            all _release@DataSink::Argument { function: release_call_site, .. } = ctx.marked_sinks(ctx.current().data_sinks(), externalizes_marker)
                .filter(|s| ctx.flows_to(ctx.id(), type_source, &(*s).clone().into()));
            any time_source = &time_sources;

            any check@DataSink::Argument { function, .. } = ctx.current().data_sinks();
            require ctx.flows_to(ctx.id(), type_source, &check.clone().into());
            require ctx.arguments(function)
                    .any(|arg| ctx.flows_to(ctx.id(), time_source, &arg.clone().into()));
            require ctx.current().ctrl_flow[&DataSource::FunctionCall(function.clone())].contains(release_call_site);
            true
        )
    });
    Ok(())
}

fn check(ctx: Arc<Context>) -> Result<()> {
    check_date_store(ctx)
}

fn check_date_store(ctx: Arc<Context>) -> Result<()> {
    let pageview_data = ctx
        .marked(marker!(pageviews))
        .map(|d| d.0)
        .collect::<Vec<_>>();
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
        let mut storing_controller = 0;
        ctx.controller_contexts().all(|ctx|{
            println!("Checking {}", ctx.describe_def(ctx.id()));
            let db_store_marker = marker!(db_store);
            ctx.report_marker_if_absent(db_store_marker);
            let time_marker = marker!(time);
            ctx.report_marker_if_absent(time_marker);
            let time_sources = ctx
                .current()
                .data_sources()
                .filter(|ds| ds.has_marker(time_marker)
                ).collect::<Vec<_>>();
            iterator_quantifiers!(
                all typ = pageview_data.iter();
                all type_ident_call_site = ctx.current().call_sites_for(*typ);
                tick(1);
                let type_ident = DataSource::FunctionCall(type_ident_call_site.clone());
                assert_warning!(ctx, !ctx.reaching(ctx.id(), &type_ident).is_empty());
                // This will be `sled::Tree::update_and_fetch` in `controller::db_utils::incr_id`
                all sink = ctx.marked_sinks(ctx.current().data_sinks(), db_store_marker);
                tick(2);
                // The next two statements are a hack. The problem here is that for call
                // `tree.update_and_fetch(key, increment)` the `time_source` flows into the
                // properly marked `key`, however `type_source` flows into `tree` and I hadn't marked
                // that because I thought they'd flow into the same argument. So
                // here I'm just hacking it together, but this should actually be done properly via marker
                let mut ty_sink = sink.clone();
                match &mut ty_sink {
                    DataSink::Argument { function: _, arg_slot } => *arg_slot = 0,
                    _ => (),
                };
                let type_is_stored = ctx.flows_to(ctx.id(), &type_ident, &ty_sink.into());
                allow !type_is_stored;
                tick(3);
                storing_controller += 1;
                let any_fits = time_sources
                    .iter()
                    .any(|time_source|
                        ctx.flows_to(ctx.id(), time_source, &sink.clone().into())
                    );
                assert_error!(ctx, any_fits, "Found no local source that influences to the pageview store {sink}");
                true
            )
        });
        // We expect this to happen in `edit_post_post`, `comment_post` and `solo_post`
        assert_error!(ctx, storing_controller == 3, format!("Not as many controllers ({storing_controller} != 3) storing pageviews as expected found, policy must be wrong"));
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
                .filter(|ds| ds.has_marker(time_marker)
                ).collect::<Vec<_>>();
            iterator_quantifiers!(
                all typ = pageview_data.iter();
                any time_source = &time_sources;
                tick(1);
                any type_ident_call_site = ctx.current().call_sites_for(*typ);
                let type_ident = DataSource::FunctionCall(type_ident_call_site.clone());
                tick(2);
                any type_source@DataSource::FunctionCall(f) = ctx.marked_sources(ctx.id(), db_access_marker);
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
        //.abort_after_analysis()
        .get_command()
        .args(["--eager-local-markers", "--inline-elision", "--", "--lib"]);
    cmd.run(dir)?.with_context(check)?;
    println!("Policy check succeeded");
    Ok(())
}
