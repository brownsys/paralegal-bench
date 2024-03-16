extern crate anyhow;
extern crate paralegal_policy;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use paralegal_policy::{
    assert_error, assert_warning,
    paralegal_spdg::{CallString, GlobalNode, Identifier, InstructionKind, Node, SPDG},
    Context, ControllerId, DefId, Diagnostics, EdgeSelection, GraphLocation, IntoIterGlobalNodes,
    Marker,
};
use petgraph::visit::EdgeRef;
use std::{collections::HashSet, sync::Arc};

macro_rules! marker {
    ($id:ident) => {{
        lazy_static::lazy_static! {
            static ref MARKER: Marker = Marker::new_intern(stringify!($id));
        }
        *MARKER
    }};
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
    fn call_sites_for<'a>(
        &'a self,
        ctr: ControllerId,
        fun: DefId,
    ) -> Box<dyn Iterator<Item = CallString> + 'a>;

    fn marked(&self, marker: Marker) -> Box<dyn Iterator<Item = GlobalNode> + '_>;
}

impl ContextExt for Context {
    fn call_sites_for<'a>(
        &'a self,
        ctrl: ControllerId,
        fun: DefId,
    ) -> Box<dyn Iterator<Item = CallString> + 'a> {
        let locs = self
            .desc()
            .instruction_info
            .iter()
            .filter(|(k, v)| matches!(v.kind, InstructionKind::FunctionCall(f) if f.id == fun))
            .map(|(k, v)| k)
            .collect::<HashSet<_>>();
        let iter = self.desc().controllers[&ctrl]
            .edges()
            .map(|e| e.weight().at)
            .filter(move |e| locs.contains(&e.leaf()));
        Box::new(iter)
    }

    fn marked(&self, marker: Marker) -> Box<dyn Iterator<Item = GlobalNode> + '_> {
        let marked_types = self.marked_type(marker);
        let iter = self
            .marked_nodes(marker)
            .chain(self.desc().controllers.iter().flat_map(|(&id, ctrl)| {
                ctrl.type_assigns
                    .iter()
                    .filter(|(_, tys)| tys.0.iter().any(|t| marked_types.contains(t)))
                    .map(move |(&n, _)| GlobalNode::from_local_node(id, n))
            }));
        Box::new(iter)
    }
}

trait CtrlExt {
    fn data_sources<'a>(&'a self) -> Box<dyn Iterator<Item = Node> + 'a>;
}

impl CtrlExt for SPDG {
    fn data_sources<'a>(&'a self) -> Box<dyn Iterator<Item = Node> + 'a> {
        Box::new(self.graph.node_indices())
    }
}

#[allow(dead_code)]
/// Not actually used, because it turns out the application doesn't do this. It just cleans up the database every 10min.
fn check_no_expired_read(ctx: Arc<Context>) -> Result<()> {
    ctx.named_policy(Identifier::new_intern("no expired read"), |ctx| {
        let expirable_data = ctx.marked_nodes(marker!(pageviews)).collect::<Vec<_>>();
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
                .map(|s| GlobalNode::from_local_node(ctx.id(), s))
                .filter(|ds| ctx.has_marker(time_marker, *ds))
                .collect::<Vec<_>>();
            all type_ident = &expirable_data;
            // Another option is to say expiration must be for all data,
            // with exceptions for certain marked types.
            all type_source = ctx.marked_nodes(db_access_marker)
                .flat_map(|input| ctx.consuming_call_sites(input))
                .flat_map(|f| ctx.inputs_of(f).iter_global_nodes().collect::<Vec<_>>())
                .filter(|f|
                    ctx.flows_to(*type_ident, *f, EdgeSelection::Data)
                );
            all release_call_site = ctx.marked_nodes(externalizes_marker)
                .filter(|s| ctx.flows_to(type_source, *s, EdgeSelection::Data));
            any time_source = &time_sources;

            any check = ctx.current().data_sinks();
            let check_node = GlobalNode::from_local_node(ctx.id(), check);
            require ctx.flows_to(type_source, check_node, EdgeSelection::Data);
            require ctx.flows_to(*time_source, check_node, EdgeSelection::Data);
            require ctx.has_ctrl_influence(check_node, release_call_site);
            true
        )
    });
    Ok(())
}

fn check_date_store(ctx: Arc<Context>) -> Result<()> {
    let pageview_data = ctx
        .marked(marker!(pageviews))
        .map(|d| d)
        .collect::<Vec<_>>();
    ctx.clone().named_policy(Identifier::new_intern("date store"), |ctx| {
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
            let db_store_marker = marker!(db_store);
            ctx.report_marker_if_absent(db_store_marker);
            let time_marker = marker!(time);
            ctx.report_marker_if_absent(time_marker);
            let time_sources = ctx
                .current()
                .data_sources()
                .filter(|ds| ctx.has_marker(time_marker, GlobalNode::from_local_node(ctx.id(), *ds) )
                ).collect::<Vec<_>>();
            iterator_quantifiers!(
                all type_ident = pageview_data.iter();
                allow type_ident.controller_id() != ctx.id();
                assert_warning!(ctx, !ctx.influencers(*type_ident, EdgeSelection::Both).next().is_none());
                // This will be `sled::Tree::update_and_fetch` in `controller::db_utils::incr_id`
                all sink = ctx.marked_nodes(db_store_marker);
                tick(2);
                // The next two statements are a hack. The problem here is that for call
                // `tree.update_and_fetch(key, increment)` the `time_source` flows into the
                // properly marked `key`, however `type_source` flows into `tree` and I hadn't marked
                // that because I thought they'd flow into the same argument. So
                // here I'm just hacking it together, but this should actually
                // be done properly via marker.
                //
                // So instead for the type ident we check that it flows into the
                // successor of the sink.
                any ty_sink = ctx.successors(sink);
                tick(3);
                let type_is_stored = ctx.flows_to(*type_ident, ty_sink, EdgeSelection::Data);
                allow !type_is_stored;
                tick(4);
                storing_controller += 1;
                let any_fits = time_sources
                    .iter()
                    .any(|time_source|
                        ctx.flows_to(GlobalNode::from_local_node(ctx.id(), *time_source), sink, EdgeSelection::Data)
                    );
                assert_error!(ctx, any_fits, "Found no local source that influences to the pageview store {sink}");
                true
            )
        });
        // We expect this to happen in `edit_post_post`, `comment_post` and `solo_post`
        assert_error!(ctx, storing_controller == 3, format!("Not as many controllers ({storing_controller} != 3) storing pageviews as expected found, policy must be wrong"));
        println!("Last seen for first policy {}", farthest.load(std::sync::atomic::Ordering::Relaxed));
    });
    Ok(())
}

fn check_expiration(ctx: Arc<Context>) -> Result<()> {
    let pageview_data = ctx
        .marked(marker!(pageviews))
        .map(|d| d)
        .collect::<Vec<_>>();
    ctx.named_policy(Identifier::new_intern("expiration check"), |ctx| {
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
            if ctx.current().name.as_str() != "user_chron_job" {
                return false;
            }
            let delete_sinks = ctx
                .marked_nodes(marker!(deletes))
                .collect::<Vec<_>>();
            let time_marker = marker!(time);
            let time_sources = ctx
                .current()
                .data_sources()
                .map(|n| GlobalNode::from_local_node(ctx.id(), n))
                .filter(|ds| ctx.has_marker(time_marker, *ds)
                ).collect::<Vec<_>>();
            iterator_quantifiers!(
                all &type_ident = pageview_data.iter();
                allow type_ident.controller_id() != ctx.id();
                tick(1);
                any type_source = ctx.marked_nodes(db_access_marker);
                tick(2);
                require ctx.flows_to(type_ident, type_source, EdgeSelection::Data);
                tick(3);
                any &delete = &delete_sinks;
                tick(4);
                require ctx.flows_to(type_source, delete, EdgeSelection::Data);
                tick(6);
                any time_check = ctx.current().all_sources().map(|n| GlobalNode::from_local_node(ctx.id(), n));
                tick(7);
                let type_flows_to_check = ctx.flows_to(type_source, time_check, EdgeSelection::Data);
                any &time_source = &time_sources;
                tick(8);
                require type_flows_to_check;
                let time_flows_to_check = ctx.flows_to(time_source, time_check, EdgeSelection::Data);
                tick(9);
                require time_flows_to_check;
                any delete_call_site = ctx.successors(delete);
                let check_ctrls_delete = ctx.has_ctrl_influence(time_check, delete_call_site);
                tick(10);
                check_ctrls_delete
            )
        });
        println!("Last seen {}", farthest.load(std::sync::atomic::Ordering::Relaxed));
        assert_error!(ctx, found, "Could not find an expiration deletion for pageview data.")
    });
    Ok(())
}

#[derive(Clone, Copy, ValueEnum)]
#[clap(rename_all = "kebab-case")]
enum Policy {
    DateStore,
    Expiration,
}

impl Policy {
    fn check(self, ctx: Arc<Context>) -> Result<()> {
        match self {
            Self::DateStore => check_date_store(ctx),
            Self::Expiration => check_expiration(ctx),
        }
    }
}

#[derive(Parser)]
struct Args {
    #[clap(long, value_enum)]
    policy: Vec<Policy>,
    #[clap(long)]
    skip_compile: bool,
}

fn main() -> Result<()> {
    let dir = "..";
    let args = Args::parse();
    let graph_loc = if args.skip_compile {
        GraphLocation::std(dir)
    } else {
        let mut cmd = paralegal_policy::SPDGGenCommand::global();
        cmd.external_annotations("external-annotations.toml")
            .abort_after_analysis()
            .get_command()
            .args(["--", "--lib"]);
        cmd.run(dir)?
    };
    let policy = if args.policy.is_empty() {
        Policy::value_variants()
    } else {
        args.policy.as_slice()
    };
    graph_loc.with_context(|ctx| {
        assert!(ctx.desc().controllers.len() > 1);
        assert!(ctx
            .desc()
            .controllers
            .values()
            .all(|v| v.graph.node_count() > 50));
        policy
            .iter()
            .cloned()
            .map(|p| p.check(ctx.clone()))
            .collect::<Result<()>>()
    })?;
    println!("Policy check succeeded");
    Ok(())
}
