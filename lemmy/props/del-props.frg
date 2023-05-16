#lang forge

open "../analysis_result.frg"
open "basic_helpers.frg"

pred properDelete[flow_set: set Ctrl->Src->CallArgument, labels: set Object->Label] {
   all c : Ctrl | {
        // for all formal parameters flowing to sink
        all fp : (fp_fun_rel.c) | some sink : labeled_objects[CallSite, db, labels] | flows_to[c, fp, sink, flow_set] implies {
            some delete_cs : labeled_objects[CallSite, ban_check, labels] | {
                // flows_to[c, fp, delete_cs, flow_set] // that formal parameter flows to ban_check
                // (delete_cs->sink) in ctrl_flow.c // there's a control flow edge from the ban_check to sink
                always_happens_before[c, fp, delete_cs, sink, flow_set] // fp always flows into a ban check before flowing into the sink
            }
        }
    }
}

test expect {
    delete: {
        properDelete[flow, labels]
    } for Flows is theorem
}