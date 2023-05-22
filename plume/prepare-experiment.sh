#cargo clean
ARGS="--external-annotations external-annotations.toml --abort-after-analysis --target plume_models --model-version v2 --skip-sigs"
#ARGS="--external-annotations external-annotations.toml --abort-after-analysis --target plume_models --inline-elision --model-version v2 --skip-sigs"
cd plume-models
cargo dfpp $ARGS --result-path analysis_result_broken.frg -- --no-default-features --features postgres 2>/dev/null
cargo dfpp $ARGS --result-path analysis_result_fixed.frg -- --no-default-features --features postgres --features delete-comments 2>/dev/null

cd ..

PROPS_DIR=../dfpp-props

echo "#lang forge\n" > check-broken.frg
cat $PROPS_DIR/sigs.frg $PROPS_DIR/basic-helpers.frg analysis_result_broken.frg props.frg >> check-broken.frg
echo "
test expect {
    properDelete : {
        property[flow, labels]
    } for Flows is theorem
}
" >> check-broken.frg
echo "#lang forge\n" > err-msg-original.frg
cat $PROPS_DIR/err_msg_sigs.frg $PROPS_DIR/basic-helpers.frg analysis_result_broken.frg props.frg $PROPS_DIR/err_msg_template_original.frg >> err-msg-original.frg
echo "
test expect {
    find_err: {
        find_erroneous_my_pred
    } for Flows is unsat
}
" >> err-msg-original.frg
echo "#lang forge\n" > err-msg-optimized.frg
cat $PROPS_DIR/err_msg_optimized_sigs.frg $PROPS_DIR/basic-helpers.frg analysis_result_broken.frg props.frg $PROPS_DIR/err_msg_template_optimized.frg >> err-msg-optimized.frg
echo "
test expect {
    find_err: {
        find_erroneous_my_pred
    } for Flows is unsat
}
" >> err-msg-optimized.frg
echo "#lang forge\n" > check-fixed.frg
cat $PROPS_DIR/sigs.frg $PROPS_DIR/basic-helpers.frg analysis_result_fixed.frg props.frg >> check-fixed.frg
echo "
test expect {
    properDelete : {
        property[flow, labels]
    } for Flows is theorem
}
" >> check-fixed.frg