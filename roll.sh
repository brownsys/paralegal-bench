ATOMIC=../../clones/atomic-server

LOG=$(pwd)/roll-log.txt
echo "" > $LOG
CODE=0

FIX_LOG=$(pwd)/fix-log.txt

while true
do
    HASH=$(git -C $ATOMIC rev-parse HEAD)
    echo $HASH >> $LOG
    echo $HASH

    cargo run --bin atomic -- $ATOMIC --buggy --annotations $(pwd)/roll-forward/atomic/external-annotations.toml -- --analyze crate::commit::Commit::apply_opts --adaptive-depth >> $LOG 2>&1
    EXIT=$?
    echo "Exit" $EXIT
    if [ $EXIT -eq 1 ];
    then
        echo $HASH "is broken" >> $FIX_LOG
    elif [ $EXIT -ne $CODE ];
    then
        exit
    fi
    NEXT=$(git -C $ATOMIC log HEAD..develop --format=%H --reverse | head -n 1)
    git -C $ATOMIC checkout --force $NEXT
done