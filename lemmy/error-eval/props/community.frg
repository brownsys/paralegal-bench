
// if there is a database write to a community, must enforce community auth check
pred property[flow: set Src->CallArgument, labels: set Object->Label] {
    all c : Ctrl | {
        all write_sink : labeled_callsites[db_community_write, labels] | {
            flowToAuth[c, write_sink, community_delete_check, flow, labels]
            flowToAuth[c, write_sink, community_ban_check, flow, labels]
        }
    }
}
