#lang forge

// open "api/comment/like.frg"
// open "api/comment/mark_as_read.frg"
// open "api/comment/save.frg"
open "api/post/like.frg"

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

    ban: {
        properBan[flow, labels]
    } for Flows is theorem

    delete: {
        properBan[flow, labels]
    } for Flows is theorem
}