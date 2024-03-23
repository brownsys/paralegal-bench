// check that for all commits that flow into stores
	// there exists a check_rights where
		// the resource flows into the check that is not influened by a new_resource
		// and the check has ctrl flow influence on the store
pred property[flow: set Ctrl->Src->CallArgument, labels: set Object->Label] {
	all c : Ctrl | all com : to_source[c, labeled_objects[Object, commit, labels]], f : labeled_objects[Sink, sink, labels] |
		flows_to[com, f, flow]
		implies {
			some chck : labeled_objects[CallArgument, check_rights, labels] | {
				flows_to[com, chck, flow]
				all new : labeled_objects[Object, new_resource, labels] | not flows_to[new, chck, flow]
				// flows_to_ctrl[c, chck, f, flow] // early return control flow influence using ? on downstream fn calls does not appear in control flow. 
			}
		}
}