set -e
#./recompile-policy.sh

DIR=$1

cp external-annotations.toml Paralegal.toml $DIR

cargo run -- $DIR