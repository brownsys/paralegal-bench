
pred property[flow_set: set Ctrl->Src->CallArgument, labels: set Object->Label] {
    // for all users that are deleted
    all c : Ctrl | 
    all u : labeled_objects_with_types[Object, user, labels] | {
        (some deleter: labeled_objects[CallArgument, to_delete, labels] | 
            (flows_to_unmodified[c, u, deleter, flow_set])) implies {
            all user_type : labeled_objects[Type, user_data, labels] | {
                some arg : labeled_objects[CallArgument, to_delete, labels] | {
                    flows_to[c, user_type, arg, flow_set]
                }
            }
        }
    }
}

//run {} for Flows

test expect {
    vacuity: {
        all c : Ctrl | 
        some u : labeled_objects_with_types[Object, user, labels] | 
        some deleter: labeled_objects[CallArgument, to_delete, labels] | 
        (flows_to_unmodified[c, u, deleter, flow])
    } for Flows is sat

    properDelete : {
        property[flow, labels]
    } for Flows is theorem
}

// sig ErroneousFlow {
//     minimal_subflow: set Src->Sink
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

// test expect {
// 	error: {
// 		find_erroneous_my_pred
// 	} for 1 ErroneousFlow for Flows is unsat
// }
