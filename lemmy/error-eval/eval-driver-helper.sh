FILE="$1-props.frg"

cd props
echo "#lang forge" > $FILE 
cat "../../../dfpp-props/basic-helpers.frg" "../../analysis_result.frg" "flowToAuth.frg" "$1.frg" "test-expect.frg" >> $FILE