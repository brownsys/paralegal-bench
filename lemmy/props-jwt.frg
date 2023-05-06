#lang forge

open "analysis_result.frg"
open "basic_helpers.frg"

// kinda unhappy because:
//      - have to pass local_user object into labeling function b/c of closure issue
//      - this has the same issue as plume where it's kinda targeted b/c of the multicrate issue, but theoretically covers like every controller (b/c they all call these functions to get user objects)
pred properAuth[flow: set Ctrl->Src->CallArgument, labels: set Object->Label] {
    all c : Ctrl | all user : (c.types).(labeled_objects[Type, local_user_view, labels]) {
        some ban : labeled_objects[CallArgument, banned_arg, labels] | {
            flows_to_ctrl[c, user, ban, flow]
        }
    }
}

test expect {

    vacuity: {
        some c : Ctrl | some (c.types).(labeled_objects[Type, local_user_view, labels])
    } for Flows is sat

    login: {
        properAuth[flow, labels]
    } for Flows is theorem
}