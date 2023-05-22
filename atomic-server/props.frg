// check that for all commits that flow into stores
	// there exists a check_rights where
		// the commit flows into a resource
		// the resource flows into the check without first flowing into new_resource
		// and the check has ctrl flow influence on the store
pred property[flow: set Ctrl->Src->CallArgument, labels: set Object->Label] {
	all c : Ctrl | all com : to_source[c, labeled_objects[Object, commit, labels]], f : labeled_objects[Sink, sink, labels] |
		flows_to[com, f, flow]
		implies {
			some chck : labeled_objects[CallArgument, check_rights, labels], res : to_source[c, labeled_objects[Object, resource, labels]] | {
				some res_sink : to_sink[c, res] | {flows_to[com, res_sink, flow]}
				flows_to[res, chck, flow]
				all new : labeled_objects[Object, new_resource, labels] | never_happens_before[c, res, new, chck, flow]
				// flows_to_ctrl[c, chck, f, flow] // early return control flow influence using ? on downstream fn calls does not appear in control flow. 
			}
		}
}