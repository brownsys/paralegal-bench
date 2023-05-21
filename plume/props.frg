
pred property[flow: set Src->CallArgument, labels: set Object->Label] {
    // for all users that are deleted
    all c : Ctrl | 
    all usr : labeled_objects_with_types[Object, user, labels] | {
        (some deleter: labeled_objects[sinks_of[c], to_delete, labels], u: to_source[c, usr] | 
            (flows_to_unmodified[u, deleter, flow])) implies {
            all user_type : labeled_objects[Type, user_data, labels] | 
            some src : to_source[c, user_type] | {
                some arg : labeled_objects[sinks_of[c], to_delete, labels] | {
                    flows_to[src, arg, flow]
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
        (flows_to_unmodified[u, deleter, flow])
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
