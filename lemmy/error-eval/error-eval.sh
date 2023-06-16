ARGS="--external-annotations external-annotations.toml --abort-after-analysis --target lemmy_api --inline-elision --model-version v2 --skip-sigs"
PARENT_DIR=error-eval
RESULTS_DIR=dfpp-results
ERR_MSG_DIR=err-msg-files
PROPS_DIR=props
DFPP_PROPS_DIR=../../../dfpp-props


cd ..
cargo dfpp $ARGS --result-path $PARENT_DIR/$RESULTS_DIR/community-block.frg -- --features "bug-1-code community-block"
cargo dfpp $ARGS --result-path $PARENT_DIR/$RESULTS_DIR/login.frg -- --features "bug-1-code bug-1-fix user-login"
cargo dfpp $ARGS --result-path $PARENT_DIR/$RESULTS_DIR/comment-delete.frg -- --features "post-bug-1 comment-delete"
cargo dfpp $ARGS --result-path $PARENT_DIR/$RESULTS_DIR/community-ban.frg -- --features "post-bug-1 community-ban"

cd $PARENT_DIR/$ERR_MSG_DIR

FILE=community-block-err-msg-original.frg
echo "#lang forge\n" > $FILE
cat $DFPP_PROPS_DIR/err_msg_sigs.frg $DFPP_PROPS_DIR/basic-helpers.frg ../$RESULTS_DIR/community-block.frg ../$PROPS_DIR/flowToAuth.frg ../$PROPS_DIR/instance.frg $DFPP_PROPS_DIR/err_msg_template_original.frg >> $FILE

FILE=community-block-err-msg-labels.frg
echo "#lang forge\n" > $FILE 
cat $DFPP_PROPS_DIR/err_msg_labels_sigs.frg $DFPP_PROPS_DIR/basic-helpers.frg ../$RESULTS_DIR/community-block.frg ../$PROPS_DIR/flowToAuth.frg ../$PROPS_DIR/instance.frg $DFPP_PROPS_DIR/err_msg_template_labels.frg >> $FILE

FILE=login-err-msg-original.frg
echo "#lang forge\n" > $FILE
cat $DFPP_PROPS_DIR/err_msg_sigs.frg $DFPP_PROPS_DIR/basic-helpers.frg ../$RESULTS_DIR/login.frg ../$PROPS_DIR/flowToAuth.frg ../$PROPS_DIR/instance.frg $DFPP_PROPS_DIR/err_msg_template_original.frg >> $FILE

FILE=login-err-msg-labels.frg
echo "#lang forge\n" > $FILE 
cat $DFPP_PROPS_DIR/err_msg_labels_sigs.frg $DFPP_PROPS_DIR/basic-helpers.frg ../$RESULTS_DIR/login.frg ../$PROPS_DIR/flowToAuth.frg ../$PROPS_DIR/instance.frg $DFPP_PROPS_DIR/err_msg_template_labels.frg >> $FILE

FILE=comment-delete-err-msg-original.frg
echo "#lang forge\n" > $FILE
cat $DFPP_PROPS_DIR/err_msg_sigs.frg $DFPP_PROPS_DIR/basic-helpers.frg ../$RESULTS_DIR/comment-delete.frg ../$PROPS_DIR/flowToAuth.frg ../$PROPS_DIR/community.frg $DFPP_PROPS_DIR/err_msg_template_original.frg >> $FILE

FILE=comment-delete-err-msg-labels.frg
echo "#lang forge\n" > $FILE 
cat $DFPP_PROPS_DIR/err_msg_labels_sigs.frg $DFPP_PROPS_DIR/basic-helpers.frg ../$RESULTS_DIR/comment-delete.frg ../$PROPS_DIR/flowToAuth.frg ../$PROPS_DIR/community.frg $DFPP_PROPS_DIR/err_msg_template_labels.frg >> $FILE

FILE=community-ban-err-msg-original.frg
echo "#lang forge\n" > $FILE
cat $DFPP_PROPS_DIR/err_msg_sigs.frg $DFPP_PROPS_DIR/basic-helpers.frg ../$RESULTS_DIR/community-ban.frg ../$PROPS_DIR/flowToAuth.frg ../$PROPS_DIR/community.frg $DFPP_PROPS_DIR/err_msg_template_original.frg >> $FILE

FILE=community-ban-err-msg-labels.frg
echo "#lang forge\n" > $FILE 
cat $DFPP_PROPS_DIR/err_msg_labels_sigs.frg $DFPP_PROPS_DIR/basic-helpers.frg ../$RESULTS_DIR/community-ban.frg ../$PROPS_DIR/flowToAuth.frg ../$PROPS_DIR/community.frg $DFPP_PROPS_DIR/err_msg_template_labels.frg >> $FILE
