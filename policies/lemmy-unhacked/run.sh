#!/usr/bin/env bash
set -e
#./recompile-policy.sh

if [ -z "$1"  ]; then 
    echo "Must provide a path as first argument"
    exit 1
elif [[ ! -d "$1" ]]; then 
    echo "Provided path must be an existing directory"
    exit 1
fi

DIR=$1

cp external-annotations.toml Paralegal.toml $DIR

cargo run -- $DIR -- --analyze "<lemmy_api_common::person::Login as lemmy_api::Perform>::perform" --adaptive-depth