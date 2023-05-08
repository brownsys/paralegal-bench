#lang forge

// open "createpostlike.frg"
// open "followcommunity.frg"
// open "lockpost.frg"
// open "login.frg"
open "stickypost.frg"

open "../basic_helpers.frg"

// obj flows to an argument labeled ls, and there's a control flow edge between the argument and return
pred flows_to_label[c: Ctrl, flow: set Ctrl->Src->CallArgument, labels: set Object->Label, obj: Object, ls: Label] {
    some cs : labeled_objects[CallArgument, ls, labels] | {
        flows_to_ctrl[c, obj, cs, flow]
        flows_to_ctrl_return[c, cs.arg_call_site, flow]
    }
}

// all objects labeled ls must flow to labels ban_check and delete_check
pred type_is_checked[c: Ctrl, flow: set Ctrl->Src->CallArgument, labels: set Object->Label, ls: Label] {
    all obj : labeled_objects_with_types[c, Type, ls, labels]| (flows_to[c, obj, Return, flow]) implies {
        flows_to_label[c, flow, labels, obj, ban_check]
        flows_to_label[c, flow, labels, obj, delete_check]
    }
}

// all types labeled "community, post, or user" that flow to the Return must flow to something labeled "ban check" and "delete check"
pred properCheck[flow: set Ctrl->Src->CallArgument, labels: set Object->Label] {
    all c : Ctrl | {
        type_is_checked[c, flow, labels, local_user_view]
        type_is_checked[c, flow, labels, community]
        type_is_checked[c, flow, labels, post]
    }
}

test expect {

    vacuity : {
        (some c : Ctrl | some user : labeled_objects_with_types[c, Type, local_user_view, labels]| (flows_to[c, user, Return, flow])) or
        (some c : Ctrl | some comm : labeled_objects_with_types[c, Type, community, labels] | (flows_to[c, comm, Return, flow])) or 
        (some c : Ctrl | some p : labeled_objects_with_types[c, Type, post, labels]| (flows_to[c, p, Return, flow]))
    } for Flows is sat

    login: {
        properCheck[flow, labels]
    } for Flows is theorem
}