#lang forge

open "../analysis_result.frg"
open "basic-helpers.frg"

// some fp flows to the auth check labeled lb, and the auth check has control flow influence on the sink
pred flowToAuth[c: Ctrl, sink: Object, lb: Label, flow_set: set Src->CallArgument, labels: set Object->Label] {
    some fp : (fp_fun_rel.c), cs : labeled_callsites[lb, labels] | {
        flows_to[fp, cs, flow_set]
        some intermediate : CallSite | {
            (some arg : arg_call_site.intermediate | flows_to_ctrl[cs, arg, flow_set])
            intermediate->sink in ctrl_flow
        }
    }
}

// if there is a database access (other than reading the user), must enforce instance auth check
pred property[flow_set: set Src->CallArgument, labels: set Object->Label] {
   all c : Ctrl | {
        all sink : labeled_callsites[db_access, labels] | {
            (sink.function)->db_user_read not in labels implies {
                flowToAuth[c, sink, instance_ban_check, flow_set, labels]
                flowToAuth[c, sink, instance_delete_check, flow_set, labels]
            }
        }
    }
}

test expect {
    prop : {
        property[flow, labels]
    } for Flows is theorem
}