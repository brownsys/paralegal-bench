#lang forge

open "../analysis_result.frg"
open "basic_helpers.frg"

pred properDelete[flow_set: set Ctrl->Src->CallArgument, labels: set Object->Label] {
   all c : Ctrl | {

        // the user is authorized
        some fp : (fp_fun_rel.c) | {
            some auth: labeled_objects[CallSite, auth_check, labels] | flows_to[c, fp, auth, flow_set]
        }

        // if there is a flow into a database function, then we check if the community is deleted/removed
        all fp : (fp_fun_rel.c) | all sink : labeled_objects[CallSite, db, labels] | flows_to[c, fp, sink, flow_set] implies {
            some delete_cs : labeled_objects[CallSite, community_delete_check, labels] | {
                (delete_cs->sink) in ctrl_flow_for_ctrl[c, flow_set] // there's a control flow edge from the delete check to sink
                always_happens_before[c, fp, delete_cs, sink, flow_set] // fp always flows into a delete check before flowing into the sink
            }
        }
    }
}

test expect {

    vacuity: {
        all c : Ctrl | some fp : (fp_fun_rel.c) | (some sink : labeled_objects[CallSite, db, labels] | flows_to[c, fp, sink, flow])
    } for Flows is sat

    delete: {
        properDelete[flow, labels]
    } for Flows is theorem
}
