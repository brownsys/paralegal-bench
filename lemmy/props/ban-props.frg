#lang forge

open "../analysis_result.frg"
open "basic_helpers.frg"

pred properBan[flow_set: set Ctrl->Src->CallArgument, labels: set Object->Label] {
    all c : Ctrl | {
        // for all formal parameters flowing to sink
        all fp : (fp_fun_rel.c) | some sink : labeled_objects[CallSite, db, labels] | flows_to[c, fp, sink, flow_set] implies {
            some ban_cs : labeled_objects[CallSite, ban_check, labels] | {
                // flows_to[c, fp, ban_cs, flow_set] // that formal parameter flows to ban_check
                // (ban_cs->sink) in ctrl_flow.c // there's a control flow edge from the ban_check to sink
                always_happens_before[c, fp, ban_cs, sink, flow_set] // fp always flows into a ban check before flowing into the sink
            }
        }
    }
}

test expect {
    ban: {
        properBan[flow, labels]
    } for Flows is theorem
}