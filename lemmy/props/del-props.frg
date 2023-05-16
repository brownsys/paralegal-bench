#lang forge

open "../analysis_result.frg"
open "basic_helpers.frg"

pred properDelete[flow: set Ctrl->Src->CallArgument, labels: set Object->Label] {
    all c : Ctrl | {
        some fp : (fp_fun_rel.c) | {
            some delete_cs : labeled_objects[CallSite, delete_check, labels] | {
                flows_to[c, fp, delete_cs, flow]
             }
        }
    }  
}

test expect {
    delete: {
        properDelete[flow, labels]
    } for Flows is theorem
}