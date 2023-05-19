#lang forge

open "../analysis_result.frg"
open "basic_helpers.frg"

// some fp flows to the auth check labeled lb, and the auth check has control flow influence on the sink
pred flowToAuth[c: Ctrl, sink: Object, lb: Label, flow_set: set Src->CallArgument, ctrl_flow : set Src -> CallSite, labels: set Object->Label] {
    some fp : (fp_fun_rel.c), cs : labeled_callsites[lb, labels] | {
        flows_to[c, fp, cs, flow_set]
        some intermediate : CallSite | {
            flows_to[c, cs, intermediate, flow_set]
            intermediate->sink in ctrl_flow
        }
    }
}

// if there is a database read (other than reading the user), must enforce instance auth check
pred properRead[flow_set: set Src->CallArgument, ctrl_flow : set Src -> CallSite, labels: set Object->Label] {
   all c : Ctrl | {
        all read_sink : labeled_callsites[db_read, labels] | {
            (read_sink.function)->db_user_read not in labels implies {
                flowToAuth[c, read_sink, instance_ban_check, flow_set, ctrl_flow, labels]
                flowToAuth[c, read_sink, instance_delete_check, flow_set, ctrl_flow, labels]
            }
        }
    }
}

test expect {
    // vacuity: {
    //     some labeled_callsites[db_read, labels]  
    // } for Flows is sat

    // v : {
    //     some read_sink : labeled_callsites[db_read, labels] | (read_sink.function)->db_user_read not in labels
    // } for Flows is sat

    dbRead: {
        properRead[flow, ctrl_flow, labels]
    } for Flows is theorem
}
