#lang forge

open "basic_helpers.frg"
open "analysis_result.frg"

pred deleteUserData[flow: set Ctrl->Src->CallArgument, labels: set Object->Label] {
    // for all users that flow into the diesel delete function
    all c : Ctrl | all u : labeled_objects_with_types[c, Object, user, labels], diesel_delete : labeled_objects[CallArgument, db_user, labels] |
    (flows_to_ctrl[c, u, diesel_delete, flow])
    implies {
        all user_type : labeled_objects[Type, user_data, labels] | { // for all Types representing user data
            some data : (c.types).user_type | { // the controller actually calls a function that returns data of this type
                some arg : labeled_objects[CallArgument, to_delete, labels] | {
                    flows_to_ctrl[c, data, arg, flow] // and that data is deleted
                }
            }
       }
    }
}

test expect {
    vacuity: {
        all c : Ctrl | some u : labeled_objects_with_types[c, Object, user, labels], diesel_delete : labeled_objects[CallArgument, db_user, labels] |
        (flows_to_ctrl[c, u, diesel_delete, flow])
    } for Flows is sat

    properDelete : {
        deleteUserData[flow, labels]
    } for Flows is theorem
}