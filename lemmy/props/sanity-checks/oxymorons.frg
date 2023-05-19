#lang forge

open "../analysis_result.frg"
open "basic_helpers.frg"

test expect {
    oxymoronRead : {
        all c : Ctrl | not(some labeled_objects[CallSite, db_read, labels])
    } is sat

    oxymoronWrite : {
        all c : Ctrl | not(some labeled_objects[CallSite, db_write, labels])
    } is sat

    oxymoronCommunityWrite : {
        all c : Ctrl | not(some write_sink : labeled_objects[CallSite, db_write, labels] | write_sink->db_community_write in labels)
    } is sat

    oxymoronNonUserRead: {
        all c : Ctrl | not(some read_sink : labeled_objects[CallSite, db_read, labels] | read_sink->db_user_read not in labels)
    } is sat
}