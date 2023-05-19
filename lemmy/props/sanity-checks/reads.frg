#lang forge

open "../analysis_result.frg"
open "basic_helpers.frg"

// vacuitys check for controllers that get a user, db read
test expect {
    vacuityRead : {
        all c : Ctrl | some labeled_objects[CallSite, db_read, labels]
    } for Flows is sat

    vacuityNonUserRead: {
        some read_sink : labeled_objects[CallSite, db_read, labels] | read_sink->db_user_read not in labels
    } for Flows is sat
}