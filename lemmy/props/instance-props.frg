#lang forge

open "../analysis_result.frg"
open "basic_helpers.frg"

// some fp flows to the auth check labeled lb, and the auth check has control flow influence on the sink
pred flowToAuth[c: Ctrl, sink: Object, lb: Label, flow_set: set Src->CallArgument, labels: set Object->Label] {
    some fp : (fp_fun_rel.c), cs : labeled_callsites[lb, labels] | {
        flows_to[c, fp, cs, flow_set]
        some intermediate : CallSite | {
            flows_to[c, cs, intermediate, flow_set]
            intermediate->sink in ctrl_flow
        }
    }
}

// if there is a database access (other than reading the user), must enforce instance auth check
pred properInstanceAccess[flow_set: set Src->CallArgument, labels: set Object->Label] {
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
    vacuityRead: {
        some read_sink : labeled_callsites[db_access, labels] | (read_sink.function)->db_user_read not in labels
    } for Flows is sat

    dbRead: {
        properInstanceAccess[flow, labels]
    } for Flows is theorem
}
