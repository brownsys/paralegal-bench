
// if there is a database access (other than reading the user), must enforce instance auth check
pred property[flow_set: set Src->CallArgument, labels: set Object->Label] {
   all c : Ctrl | {
        all sink : labeled_callsites[db_access, labels] | {
            no ((sink.function + sink)->db_user_read & labels) implies {
                flowToAuth[c, sink, instance_ban_check, flow_set, labels]
                flowToAuth[c, sink, instance_delete_check, flow_set, labels]
            }
        }
    }
}
