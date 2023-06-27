
// some fp flows to the auth check labeled lb, and the auth check has control flow influence on the sink
pred flowToAuth[c: Ctrl, sink: Object, lb: Label, flow_set: set Src->CallArgument, labels: set Object->Label] {
    some fp : (fp_fun_rel.c), cs : (labeled_callsites[lb, labels] + labeled_objects[Object, lb, labels]) | {
        flows_to[fp, cs, flow_set]
        some intermediate : CallSite | {
            (some arg : arg_call_site.intermediate | flows_to_ctrl[cs, arg, flow_set])
            intermediate->sink in ctrl_flow
        }
    }
}
