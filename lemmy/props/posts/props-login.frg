#lang forge

open "login.frg"
open "basic_helpers.frg"

// no successful login for a user that is banned
pred properLogin[flow: set Ctrl->Src->CallArgument, labels: set Object->Label] {
    all c : Ctrl | all user : labeled_objects_with_types[c, Type, local_user_view, labels]| (flows_to[c, user, Return, flow]) implies {
        some ban_cs : labeled_objects[CallArgument, banned_arg, labels] | {
            flows_to_ctrl[c, user, ban_cs, flow]
            flows_to_ctrl_return[c, ban_cs.arg_call_site, flow]
        }
    }
}

test expect {

    vacuity : {
        some c : Ctrl | some user : labeled_objects_with_types[c, Type, local_user_view, labels]| (flows_to[c, user, Return, flow])
    } for Flows is sat

    login: {
        properLogin[flow, labels]
    } for Flows is theorem
}