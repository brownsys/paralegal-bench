#lang forge

open "basic_helpers.frg"
open "analysis_result.frg"

pred deleteUserData[flow_set: set Ctrl->Src->CallArgument, labels: set Object->Label] {
    // for all users that are deleted
    all c : Ctrl | all u : labeled_objects_with_types[c, Object, user, labels] | some deleter: labeled_objects[CallArgument, to_delete, labels] | 
    (flows_to[c, u, deleter, flow_set])
    implies {
        all user_type : labeled_objects[Type, user_data, labels] | { // for all Types representing user data
			some arg : labeled_objects[CallArgument, to_delete, labels] | {
				flows_to_ctrl[c, user_type, arg, flow_set] // that data is deleted
			}
		}
    }
}

test expect {
    vacuity: {
        all c : Ctrl | some u : labeled_objects_with_types[c, Object, user, labels] | some deleter: labeled_objects[CallArgument, to_delete, labels] | 
        (flows_to[c, u, deleter, flow])
    } for Flows is sat

    properDelete : {
        deleteUserData[flow, labels]
    } for Flows is theorem
}
// run {} for Flows

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