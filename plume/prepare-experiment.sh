#cargo clean
#ARGS="--external-annotations external-annotations.toml --abort-after-analysis --target plume_models --model-version v2 --skip-sigs"
ARGS="--external-annotations external-annotations.toml --abort-after-analysis --target plume_models --inline-elision --model-version v2 --skip-sigs"
cd plume-models
cargo dfpp $ARGS --result-path analysis_result_broken.frg -- --no-default-features --features postgres 2>/dev/null
cargo dfpp $ARGS --result-path analysis_result_fixed.frg -- --no-default-features --features postgres --features delete-comments 2>/dev/null

cd ..

PROPS_DIR=../dfpp-props

FILE=check-broken.frg
echo "#lang forge\n" > $FILE
cat $PROPS_DIR/sigs.frg $PROPS_DIR/basic-helpers.frg analysis_result_broken.frg props.frg >> $FILE
echo "
test expect {
    properDelete : {
        property[flow, labels]
    } for Flows is theorem
}
" >> $FILE

FILE=err-msg-original.frg
echo "#lang forge\n" > $FILE
cat $PROPS_DIR/err_msg_sigs.frg $PROPS_DIR/basic-helpers.frg analysis_result_broken.frg props.frg $PROPS_DIR/err_msg_template_original.frg >> $FILE


FILE=err-msg-optimized.frg
echo "#lang forge\n" > $FILE
cat $PROPS_DIR/err_msg_optimized_sigs.frg $PROPS_DIR/basic-helpers.frg analysis_result_broken.frg props.frg $PROPS_DIR/err_msg_template_optimized.frg >> $FILE
FILE=err-msg-labels.frg
echo "#lang forge\n" > $FILE 
cat $PROPS_DIR/err_msg_labels_sigs.frg $PROPS_DIR/basic-helpers.frg analysis_result_broken.frg props.frg $PROPS_DIR/err_msg_template_labels.frg >> $FILE


FILE=check-fixed.frg
echo "#lang forge\n" > $FILE
cat $PROPS_DIR/sigs.frg $PROPS_DIR/basic-helpers.frg analysis_result_fixed.frg props.frg >> $FILE
echo "
test expect {
    properDelete : {
        property[flow, labels]
    } for Flows is theorem
}
" >> $FILE