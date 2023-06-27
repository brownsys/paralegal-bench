#lang forge

open "../analysis_result.frg"
open "basic-helpers.frg"

// some fp flows to the auth check labeled lb, and the auth check has control flow influence on the sink
pred flowToAuth[c: Ctrl, sink: Object, lb: Label, flow_set: set Src->CallArgument, labels: set Object->Label] {
    some fp : (fp_fun_rel.c), cs : (labeled_callsites[lb, labels] + labeled_objects[Object, lb, labels]) | {
        flows_to[fp, cs, flow_set]
        some intermediate : CallSite | {
            (some arg : arg_call_site.intermediate | flows_to_ctrl[cs, arg, flow_set])
            intermediate->sink in ctrl_flow
        }
    }
}

// if there is a database write to a community, must enforce community auth check
pred property[flow: set Src->CallArgument, labels: set Object->Label] {
    all c : Ctrl | {
        all write_sink : labeled_callsites[db_community_write, labels] | {
            flowToAuth[c, write_sink, community_delete_check, flow, labels]
            flowToAuth[c, write_sink, community_ban_check, flow, labels]
        }
    }
}

test expect {
    prop : {
        property[flow, labels]
    } for Flows is theorem
}

