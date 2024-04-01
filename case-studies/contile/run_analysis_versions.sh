ARGS="--remove-inconsequential-calls aggressive --external-annotations external-annotations.toml --abort-after-analysis --verbose --dump-serialized-non-transitive-graph --drop-poll"
#ARGS="--external-annotations external-annotations.toml --abort-after-analysis --verbose --dump-serialized-non-transitive-graph --drop-poll"
PREFIX=""
set -e
cargo dfpp $ARGS --result-path analysis_result_${PREFIX}broken.frg -- --lib --features leak > analysis_log_${PREFIX}broken.txt
mv get_tiles.ntgb.json get_tiles_${PREFIX}broken.ntgb.json

cargo dfpp $ARGS --result-path analysis_result_${PREFIX}fixed.frg -- --lib > analysis_log_${PREFIX}fixed.txt
mv get_tiles.ntgb.json get_tiles_${PREFIX}fixed.ntgb.json