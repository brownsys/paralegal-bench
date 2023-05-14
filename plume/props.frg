#lang forge

open "basic_helpers.frg"
open "analysis_result.frg"

pred deleteUserData[flow_set: set Ctrl->Src->CallArgument, labels: set Object->Label] {
    // for all users that flow into the diesel delete function
    all c : Ctrl | all u : labeled_objects_with_types[c, Object, user, labels], diesel_delete : labeled_objects[CallArgument, db_user, labels] |
    (flows_to_ctrl[c, u, diesel_delete, flow_set])
    implies {
        all user_type : labeled_objects[Type, user_data, labels] | { // for all Types representing user data
            some data : (c.types).user_type | { // the controller actually calls a function that returns data of this type
                some arg : labeled_objects[CallArgument, to_delete, labels] | {
                    flows_to_ctrl[c, data, arg, flow_set] // and that data is deleted
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

// sig ErroneousFlow {
//     minimal_subflow: set CallSite->CallArgument
// }

// pred find_erroneous_my_pred_int[ef: ErroneousFlow] {
//     some c : Ctrl | {
// 		(c->ef.minimal_subflow in flow)
//     (not deleteUserData[flow, labels])
//     (deleteUserData[(flow - (c->ef.minimal_subflow)), labels]) }
// }

// pred find_erroneous_my_pred {
//     some ef: ErroneousFlow {
//         find_erroneous_my_pred_int[ef]
//     }
// }

// run {
//     find_erroneous_my_pred
// } for 1 ErroneousFlow for Flows