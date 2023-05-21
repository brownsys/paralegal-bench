// check that for all commits that flow into stores
	// there exists a check_rights where
		// the commit flows into a resource
		// the resource flows into the check without first flowing into new_resource
		// and the check has ctrl flow influence on the store
pred property[flow_set: set Ctrl->Src->CallArgument, labels: set Object->Label] {
	all c : Ctrl | all com : to_source[c, labeled_objects[Object, commit, labels]], f : labeled_objects[Sink, sink, labels] |
		flows_to[com, f, flow_set]
		implies {
			some chck : labeled_objects[CallArgument, check_rights, labels], res : to_source[c, labeled_objects[Object, resource, labels]] | {
				flows_to[com, to_sink[c, res], flow_set]
				flows_to[res, chck, flow_set]
				all new : labeled_objects[Object, new_resource, labels] | never_happens_before[c, res, new, chck, flow_set]
				// flows_to_ctrl[c, chck, f, flow_set] // early return control flow influence using ? on downstream fn calls does not appear in control flow. 
			}
		}
}