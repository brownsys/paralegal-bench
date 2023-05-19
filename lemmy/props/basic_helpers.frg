#lang forge

open "../analysis_result.frg"

sig ErroneousFlow {
    minimal_subflow: set CallSite->CallArgument
}

sig IncompleteLabel {
    missing_labels: set CallArgument->Label
}

fun to_source[c: one Ctrl, o: one Type + Src + CallSite] : Src {
    {src : Src |
        (o in Type and src->o in types)  or 
        (o = src or src->o in arg_call_site)
    }
}

fun to_sink[c: one Ctrl, o: one Type + Src] : Sink {
    arg_call_site.(to_source[o])
}

// just c's flows
fun flow_for_ctrl[c: one Ctrl, flow_set : set Src->Sink] : set Src->Sink {
    ((c.calls + fp_fun_rel.c)->Sink) & flow_set
}

// just c's ctrl flow
fun ctrl_flow_for_ctrl[c: one Ctrl, ctrl_flow : set Src->CallSite] : set Src->Sink {
    (c.calls + fp_fun_rel.c)->CallSite & ctrl_flow
    
}

pred flows_to[cs: Ctrl, o: one Type + Src + CallSite, f : (CallArgument + CallSite), flow_set: set Src->Sink] {
    some c: cs |
    let a = to_source[c, o] | {
        let b = flow_for_ctrl[c, flow_set] | {
            some (a.b) // a exists in cs
            and (a -> f in ^(b + arg_call_site))
        }
    }
}

fun labeled_objects[obs: Object, ls: Label, labels_set: set Object->Label] : set Object {
    labels_set.ls & obs
}

// Returns all objects labelled either directly or indirectly
// through types.
fun labeled_objects_with_types[cs: Ctrl, obs: Object, ls: Label, labels_set: set Object->Label] : set Object {
    labeled_objects[obs, ls, labels_set] + (cs.types).(labeled_objects[obs, ls, labels_set])
}

// verifies that for an type o, it flows into first before flowing into next
pred always_happens_before[cs: Ctrl, o: Object, first: (CallArgument + CallSite), next: (CallArgument + CallSite), flow_set: set Src->CallArgument] {
    not (
        some c: cs | 
        some a: Object | {
            o = a or (o in Type and a->o in types and (a in fp_fun_rel.c or a in c.calls))
            a -> next in ^(flow_for_ctrl[c, flow_set] + arg_call_site - 
                        (first->CallSite + CallArgument->first))
        }
    )
}

fun arguments[f : CallSite] : set CallArgument {
    arg_call_site.f
}

// This predicate needs work.  Right now it just asserts
// that this call site is not influenced by control flow, but it should actually
// ensure that function for
// cs is called in every control flow path through c.
pred unconditional[c: one Ctrl, cs: one CallSite] {
    no c.ctrl_flow.cs
}

// FIXME: adjust for arity 2 relations
pred flows_to_unmodified[cs: Ctrl, o: one Type + Src + CallSite, f : (CallArgument + CallSite), flow_set: set Ctrl->Src->CallArgument] {
	some c: cs |
    let a = to_source[c, o] | {
        some c.flow_set[a] // a exists in cs
        and a -> f in c.flow_set
    }
}

pred flows_to_without[cs: Ctrl, o: one Type + Src + CallSite, f : (CallArgument + CallSite), without: (CallArgument + CallSite), flow_set: set Ctrl->Src->CallArgument] {
    some c: cs |
    let a = to_source[c, o] | {
        some c.flow_set[a] // a exists in cs
        and (a -> f in ^(c.flow_set + arg_call_site - (without->CallSite + CallArgument->without)))
    }
}

pred flows_to_ctrl[cs: Ctrl, o: one Type + Src + CallSite, f : (CallArgument + CallSite), flow_set: set Ctrl->Src->CallArgument] {
    some c: cs |
    some a : Src | {
        o = a or o in Type and a->o in c.types
        ((a -> f in ^(c.flow_set + c.ctrl_flow + arg_call_site))
		or
		(some f.arg_call_site and (a -> f.arg_call_site in ^(c.flow_set + c.ctrl_flow + arg_call_site))))
    }
}



