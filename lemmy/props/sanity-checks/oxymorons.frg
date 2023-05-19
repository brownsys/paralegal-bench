#lang forge

open "../analysis_result.frg"
open "basic_helpers.frg"

test expect {
    oxymoron: {
        some c : Ctrl | not (some fp : (fp_fun_rel.c) | (some read_user_sink : labeled_objects[CallSite, db_user_read, labels] | flows_to[c, fp, read_user_sink, flow]))
    } is sat
}