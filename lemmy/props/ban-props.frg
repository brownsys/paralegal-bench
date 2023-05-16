#lang forge

open "../analysis_result.frg"
open "basic_helpers.frg"

pred properBan[flow: set Ctrl->Src->CallArgument, labels: set Object->Label] {
    all c : Ctrl | {
        some fp : (fp_fun_rel.c) | {
            some ban_cs : labeled_objects[CallSite, ban_check, labels] | {
                flows_to[c, fp, ban_cs, flow]
            }
        }
    }
}

test expect {
    ban: {
        properBan[flow, labels]
    } for Flows is theorem
}