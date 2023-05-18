#lang forge

open "../small.frg"
open "basic_helpers.frg"

// fp flows to the auth checked labeled lb, and the auth check has control flow influence on the sink
pred flowToAuth[c: Ctrl, fp: Object, sink: Object, lb: Label, flow_set: set Src->CallArgument, ctrl_flow : set Src -> CallSite, labels: set Object->Label] {
    some cs : labeled_objects[CallSite, lb, labels] | {
        flows_to[c, fp, cs, flow_set]
        some intermediate : CallSite | {
            flows_to[c, cs, intermediate, flow_set]
            intermediate->sink in ctrl_flow_for_ctrl[c, ctrl_flow]
        }
    }
}

// Enforces 1) instance and 2) community auth checking for database access
pred properAuth[flow_set: set Src->CallArgument, ctrl_flow : set Src -> CallSite, labels: set Object->Label] {
   all c : Ctrl | {
        // if there is a database read (other than reading the user), must enforce instance auth check
        all fp : (fp_fun_rel.c) | all read_sink : labeled_objects[CallSite, db_read, labels] | (flows_to[c, fp, read_sink, flow_set] and (read_sink->db_user_read not in labels)) implies {
            flowToAuth[c, fp, read_sink, instance_auth_check, flow_set, ctrl_flow, labels]
        }

        // if there is a database write, must enforce instance and community auth check
        all fp : (fp_fun_rel.c) | all write_sink : labeled_objects[CallSite, db_write, labels] | flows_to[c, fp, write_sink, flow_set] implies {
            flowToAuth[c, fp, write_sink, community_delete_check, flow_set, ctrl_flow, labels]
            flowToAuth[c, fp, write_sink, community_ban_check, flow_set, ctrl_flow, labels]
            flowToAuth[c, fp, write_sink, instance_auth_check, flow_set, ctrl_flow, labels]
        }
    }
}

test expect {

    vacuity: {
        all c : Ctrl | some fp : (fp_fun_rel.c) | (some read_user_sink : labeled_objects[CallSite, db_user_read, labels] | flows_to[c, fp, read_user_sink, flow])
    } for Flows is sat

    oxymoron: {
        some c : Ctrl | not (some fp : (fp_fun_rel.c) | (some read_user_sink : labeled_objects[CallSite, db_user_read, labels] | flows_to[c, fp, read_user_sink, flow]))
    } is sat

    delete: {
        properAuth[flow, ctrl_flow, labels]
    } for Flows is theorem
}
