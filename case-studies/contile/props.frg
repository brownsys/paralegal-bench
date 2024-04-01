#lang forge
open "analysis_result_fixed.frg"

fun to_source[c: one Ctrl, o: one Type + Src] : Src {
    {src : Src |
        o in Type and src->o in c.types or o = src
    }
}

fun to_sink[c: one Ctrl, o: one Type + Src] : Sink {
    arg_call_site.(to_source[o])
}

// This predicate needs work.  Right now it just asserts
// that this call site is not influenced by control flow, but it should actually
// ensure that function for
// cs is called in every control flow path through c.
pred unconditional[c: one Ctrl, cs: one CallSite] {
    no c.ctrl_flow.cs
}

pred flows_to[cs: Ctrl, o: one Type + Src, f : (CallArgument + CallSite)] {
    some c: cs |
    let a = to_source[c, o] | {
        some c.flow[a] // a exists in cs
        and (a -> f in ^(c.flow + arg_call_site))
    }
}

pred flows_to_ctrl[cs: Ctrl, o: Object, f : CallArgument] {
    some c: cs |
    some a : Src | {
        o = a or o in Type and a->o in c.types
        a -> f in ^(c.flow + c.ctrl_flow + arg_call_site)
    }
}

fun labeled_objects[obs: Object, ls: Label] : set Object {
    labels.ls & obs
}

// Returns all objects labelled either directly or indirectly
// through types.
fun labeled_objects_with_types[cs: Ctrl, obs: Object, ls: Label] : set Object {
    labeled_objects[obs, ls] + (cs.types).(labeled_objects[obs, ls])
}

// verifies that for an type o, it flows into first before flowing into next
pred always_happens_before[cs: Ctrl, o: Object, first: (CallArgument + CallSite), next: (CallArgument + CallSite)] {
    not (
        some c: cs | 
        some a: Object | {
            o = a or o in Type and a->o in c.types
            a -> next in ^(c.flow + arg_call_site - 
                (first->CallSite + CallArgument->first))
        }
    )
}

fun arguments[f : CallSite] : set CallArgument {
    arg_call_site.f
}

//run {} for Flows

expect {
    personal_tags_not_in_metrics: {
        all a_sink: function.(labeled_objects[Function, sink]), a_sensitive: function.(labeled_objects[Function, sensitive]) |
        no ctrl: Ctrl, intersection: CallSite |
        flows_to[ctrl, a_sink, intersection] && flows_to[ctrl, a_sensitive, intersection]
    } for Flows is theorem
}

test expect {
    personal_tags_not_in_metrics_send: {
        all personal: (labeled_objects[CallArgument, sensitive]).arg_call_site, metrics_send: labeled_objects[CallArgument, metrics_server] |
            no ctrl: Ctrl |
                flows_to[ctrl, personal, metrics_send]
    } for Flows is theorem
}

