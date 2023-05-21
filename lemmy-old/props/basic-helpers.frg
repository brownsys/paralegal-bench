#lang forge 

open "../analysis_result.frg"

fun labeled_callsites[ls: Label, labels_set: set Object->Label] : CallSite {
    // (CallSite->labeled_objects[Function, ls, labels_set]) & function
    function.(labeled_objects[Function, ls, labels_set])
}

fun to_source[c: Ctrl, o: one Type + Src + CallSite] : Src {
    {src : sources_of[c] |
        o in Type and src->o in types or 
		(o = src 
		or o->src in arg_call_site)
    }
}

fun to_sink[c: Ctrl, o: one Type + Src] : Sink {
    { sink : sinks_of[c] |
        o in Type and sink->o in types or 
        (o = sink or o->sink in arg_call_site )
    }
}

fun sources_of[c: Ctrl]: set Src {
    c.calls + fp_fun_rel.c
}

fun sinks_of[c: Ctrl]: set Sink {
    arg_call_site.(c.calls) + Return
}

// This predicate needs work.  Right now it just asserts
// that this call site is not influenced by control flow, but it should actually
// ensure that function for
// cs is called in every control flow path through c.
pred unconditional[cs: one CallSite] {
    no ctrl_flow.cs
}

// just c's flows
fun flow_for_ctrl[c: Ctrl, flow_set : set Src->Sink] : set Src->Sink {
    ((c.calls + fp_fun_rel.c)->Sink) & flow_set
}

// just c's ctrl flow
fun ctrl_flow_for_ctrl[c: Ctrl, ctrl_flow : set Src->CallSite] : set Src->Sink {
    (c.calls + fp_fun_rel.c)->CallSite & ctrl_flow
    
}

pred flows_to[src: one Src, f : one Sink, flow_set: Src->CallArgument] {
    (src -> f in ^(flow_set + arg_call_site))
}

fun labeled_objects[obs: Object, ls: Label, labels_set: set Object->Label] : set Object {
    labels_set.ls & obs
}

// Returns all objects labelled either directly or indirectly
// through types.
fun labeled_objects_with_types[obs: Object, ls: Label, labels_set: set Object->Label] : set Object {
    labeled_objects[obs, ls, labels_set] + types.(labeled_objects[obs, ls, labels_set])
}

// verifies that for an type o, it flows into first before flowing into next
pred always_happens_before[cs: Ctrl, o: Object, first: (CallArgument + CallSite), next: (CallArgument + CallSite), flow_set: set Src->CallArgument] {
    not (
        let a = to_source[cs, o] | {
            a -> next in ^(flow_set + arg_call_site - 
                (first->CallSite + CallArgument->first))
        }
    )
}

// verifies that for an object o
pred never_happens_before[cs: Ctrl, in_obj: Object, first: (CallArgument + CallSite), next: (CallArgument + CallSite), flow_set: set Ctrl->Src->CallArgument] {
	not (
		some c: cs | some o: to_source[c, in_obj], f: to_source[c, first], n: to_sink[c, next] | some fsnk: to_sink[c, f] | {
			flows_to[o, fsnk, flow_set]
			flows_to[f, n, flow_set]
		}
	)
}

fun arguments[f : CallSite] : set CallArgument {
    arg_call_site.f
}

pred flows_to_ctrl[src: one Src, f : one Sink, flow_set: set Src->CallArgument] {
    let total_flow = ^(flow_set + ctrl_flow + arg_call_site) |
    ((src -> f in total_flow)
    or
    (some f.arg_call_site and (src -> f.arg_call_site in total_flow)))
}

pred flows_to_unmodified[o: one Src + CallSite, f : (CallArgument + CallSite), flow_set: set Ctrl->Src->CallArgument] {
    o -> f in flow_set
}

pred flows_to_without[o: one Src + CallSite, f : (CallArgument + CallSite), without: (CallArgument + CallSite), flow_set: set Ctrl->Src->CallArgument] {
    (o -> f in ^(flow_set + arg_call_site - (without->CallSite + CallArgument->without)))
}

fun path_of[o: one Src + Sink, f: Src + Sink, flow: set Src->Sink]: set Src + Sink {
    { s: Src + Sink | o->s + s->f in ^(flow + arg_call_site)}
}