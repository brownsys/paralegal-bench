#lang forge

open "analysis_result.frg"
open "basic_helpers.frg"

// check that for all resources that flow into stores
	// there exists a check_rights whwere
		// the resource flows into the check without first flowing into new_resource
		// and the check has ctrl flow influence on the store
pred checks_old_rights_before_storage[flow_set: set Ctrl->Src->CallArgument] {
	all c : Ctrl, com : labeled_objects[FormalParameter + Type, resource, labels], f : labeled_objects[Sink, sink, labels] |
		flows_to[c, com, f, flow_set]
		implies {
			some chck : labeled_objects[CallArgument, check_rights, labels] | {
				flows_to_without[c, com, chck, labeled_objects[Object, new_resource, labels], flow_set]
				flows_to_ctrl[c, chck, f, flow_set]
			}
		}
}

test expect {
    check_rights: {
        checks_old_rights_before_storage[flow]
    } for Flows is theorem
}