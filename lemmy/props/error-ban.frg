open "ban-props.frg"

sig ErroneousFlow {
    minimal_subflow: set Src->Sink
}

// Original version
pred find_erroneous_proper_ban_int[ef: ErroneousFlow] {
    some c : Ctrl | {
		(c->ef.minimal_subflow in flow)
    (not properBan[flow])
    (properBan[(flow - (c->ef.minimal_subflow))]) }
}

pred find_erroneous_my_pred {
    some ef1: ErroneousFlow {
        find_erroneous_proper_ban_int[ef1]
    }
}

run {
    find_erroneous_my_pred
} for 1 ErroneousFlow for Flows


// "Optimized" version
pred find_erroneous_proper_ban_int[ef: ErroneousFlow] {
    some c : Ctrl | {
		(c->ef.minimal_subflow in flow)
    (not properBan[flow])
    (properBan[(flow - (c->ef.minimal_subflow))]) }
}

pred find_erroneous_my_pred {
    some ef1: ErroneousFlow {
        find_erroneous_proper_ban_int[ef1]

				no ef2: ErroneousFlow | {
            find_erroneous_proper_ban_int[ef2]
            #(ef2.minimal_subflow) < #(ef1.minimal_subflow)
        }
    }
}

run {
    find_erroneous_my_pred
} for exactly 2 ErroneousFlow for Flows

// "Minimal" version
pred find_erroneous_proper_ban_int[ms: set Src->Sink] {
    some c : Ctrl | {
		(c->ms in flow)
    (not properBan[flow])
    (properBan[(flow - (c->ms))]) }
}

pred find_erroneous_my_pred {
    some ef1: ErroneousFlow {
        find_erroneous_proper_ban_int[ef1.minimal_subflow]

        no src: Src, sink: Sink {
						src->sink in ef1.minimal_subflow
            find_erroneous_proper_ban_int[ef1.minimal_subflow - src->sink]
        }
    }
}

run {
    find_erroneous_my_pred
} for 1 ErroneousFlow for Flows