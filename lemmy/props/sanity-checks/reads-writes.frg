#lang forge

open "../../analysis_result.frg"
open "../basic_helpers.frg"

// vacuitys check for controllers that get a user, db read and db write to communities
test expect {
    vacuityRead : {
        all c : Ctrl | some labeled_objects[CallSite, db_read, labels]
    } for Flows is sat

    vacuityWrite : {
        all c : Ctrl | some labeled_objects[CallSite, db_write, labels]
    } for Flows is sat

    vacuityCommunityWrite : {
        all c : Ctrl | some write_sink : labeled_objects[CallSite, db_write, labels] | write_sink->db_community_write in labels
    } for Flows is sat

    vacuityNonUserRead: {
        all c : Ctrl | some read_sink : labeled_objects[CallSite, db_read, labels] | read_sink->db_user_read not in labels
    } for Flows is sat
}