set -e
HERE=$(pwd)
cd ../../../paralegal-compiler

cargo run -- $HERE/policy.txt -o $HERE/src/policy.rs