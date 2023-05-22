#lang forge

open "analysis_result.frg"
open "../dfpp-props/basic_helpers.frg"

// check that for all commits that flow into stores
	// there exists a check_rights where
		// the commit flows into a resource
		// the resource flows into the check without first flowing into new_resource
		// and the check has ctrl flow influence on the store
pred property[flow_set: set Ctrl->Src->CallArgument] {
	all c : Ctrl, com : labeled_objects[Object, commit, labels], f : labeled_objects[Sink, sink, labels] |
		flows_to[c, com, f, flow_set]
		implies {
			some chck : labeled_objects[CallArgument, check_rights, labels], res : labeled_callsites[resource, labels] | {
				flows_to[c, com, res, flow_set]
				flows_to[c, res, chck, flow_set]
				all new : labeled_objects[Object, new_resource, labels] | never_happens_before[c, res, new, chck, flow_set]
				// flows_to_ctrl[c, chck, f, flow_set] // early return control flow influence using ? on downstream fn calls does not appear in control flow. 
			}
		}
}

test expect {
	vacuity: {
		some c : Ctrl, com : labeled_objects[Object, commit, labels], f : labeled_objects[Sink, sink, labels] |
		flows_to[c, com, f, flow]
	} for Flows is sat

    check_rights: {
        property[flow]
    } for Flows is theorem
}

// run {} for Flows
