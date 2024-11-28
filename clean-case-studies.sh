#!/bin/bash

# Change to the case-studies directory
cd case-studies || exit 1

# Find all directories containing Cargo.toml and run cargo clean in each
find . -maxdepth 2 -mindepth 2 -name Cargo.toml -exec dirname {} \; | while read -r dir; do
    echo "Cleaning in $dir"
    (cd "$dir" && cargo clean)
    echo "----------------------------------------"
done

echo "Finished cleaning all Rust projects"