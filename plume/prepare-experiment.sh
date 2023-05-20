#cargo clean
cd plume-models
cargo dfpp --external-annotations external-annotations.toml --abort-after-analysis --target plume_models --inline-elision --model-version v2 --result-path analysis_result_broken.frg --skip-sigs -- --no-default-features --features postgres 2>/dev/null
cargo dfpp --external-annotations external-annotations.toml --abort-after-analysis --target plume_models --inline-elision --model-version v2 --result-path analysis_result_fixed.frg --skip-sigs -- --no-default-features --features postgres --features delete-comments 2>/dev/null

cd ..

PROPS_DIR=../dfpp-props

echo "#lang forge\n" > check-broken.frg
cat $PROPS_DIR/sigs.frg $PROPS_DIR/basic-helpers.frg analysis_result_broken.frg props.frg >> check-broken.frg
echo "#lang forge\n" > check-fixed.frg
cat $PROPS_DIR/sigs.frg $PROPS_DIR/basic-helpers.frg analysis_result_fixed.frg props.frg >> check-fixed.frg