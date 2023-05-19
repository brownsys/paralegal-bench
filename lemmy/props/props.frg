#lang forge

open "../analysis_result.frg"
open "basic_helpers.frg"

// some fp flows to the auth check labeled lb, and the auth check has control flow influence on the sink
pred flowToAuth[c: Ctrl, sink: Object, lb: Label, flow_set: set Src->CallArgument, ctrl_flow : set Src -> CallSite, labels: set Object->Label] {
    some fp : (fp_fun_rel.c), cs : labeled_objects[CallSite, lb, labels] | {
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
        all read_sink : labeled_objects[CallSite, db_read, labels] | {
            read_sink->db_user_read not in labels implies {
                flowToAuth[c, read_sink, instance_ban_check, flow_set, ctrl_flow, labels]
                flowToAuth[c, read_sink, instance_delete_check, flow_set, ctrl_flow, labels]
            }
        }
    }
}

pred properWrite[flow_set: set Src->CallArgument, ctrl_flow : set Src -> CallSite, labels: set Object->Label] {
    all c : Ctrl | {
        all write_sink : labeled_objects[CallSite, db_write, labels] | {
            // if there is a database write, must enforce instance auth check
            flowToAuth[c, write_sink, instance_delete_check, flow_set, ctrl_flow, labels]
            flowToAuth[c, write_sink, instance_ban_check, flow_set, ctrl_flow, labels]

            // if the write is to a community, must also enforce community auth check
            write_sink->db_community_write in labels implies {
                flowToAuth[c, write_sink, community_delete_check, flow_set, ctrl_flow, labels]
                flowToAuth[c, write_sink, community_ban_check, flow_set, ctrl_flow, labels]
            }
        }
    }
}

test expect {

    dbRead: {
        properRead[flow, ctrl_flow, labels]
    } for Flows is theorem

     dbWrite: {
        properWrite[flow, ctrl_flow, labels]
    } for Flows is theorem
}
