#lang forge

open "analysis_result.frg"
open "basic_helpers.frg"

// check that for all commits that flow into stores
	// there exists a check_rights where
		// the commit flows into a resource
		// the resource flows into the check without first flowing into new_resource
		// and the check has ctrl flow influence on the store
pred checks_old_rights_before_storage[flow_set: set Ctrl->Src->CallArgument] {
	all c : Ctrl, com : labeled_objects[Object, commit, labels], f : labeled_objects[Sink, sink, labels] |
		flows_to[c, com, f, flow_set]
		implies {
			some chck : labeled_objects[CallArgument, check_rights, labels], res : labeled_objects[Object, resource, labels] | {
				flows_to[c, com, res, flow_set]
				flows_to_without[c, res, chck, labeled_objects[Object, new_resource, labels], flow_set]
				// flows_to_ctrl[c, chck, f, flow_set] // early return control flow influence using ? on downstream fn calls does not appear in control flow. 
			}
		}
}

test expect {
	vacuity: {
		some c : Ctrl, com : labeled_objects[Object, resource, labels], f : labeled_objects[Sink, sink, labels] |
		flows_to[c, com, f, flow]
	} for Flows is sat

    check_rights: {
        checks_old_rights_before_storage[flow]
    } for Flows is theorem
}

// run {} for Flows