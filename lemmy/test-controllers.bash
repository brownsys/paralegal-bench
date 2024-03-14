#!/bin/bash

# Thx chatgpt for the raw version of this

controllers=(
    "registration-approve"
    "site-read"
    "comment-create"
)

# Log file path
log_file="log.txt"
err_log="err.txt"

rm $log_file
rm $err_log

# Function to execute commands with timeout
execute_command() {
    local controller=$1
    echo "$controller" >> "$err_log"
    timeout 5m cargo paralegal-flow --abort-after-analysis --target lemmy_api --verbose -- --features $controller >> "$log_file" 2>> "$err_log"
    local exit_code=$?
    if [ $exit_code -eq 124 ]; then
        echo "$(date) - Timeout reached for command: $controller" >> "$log_file"
    elif [ $exit_code -eq 137 ]; then
        echo "$(date) - Analysis for '$controller' killed by the operating system due to out-of-memory." >> "$log_file"
    else
        echo "$(date) - Exit code for controller '$controller': $exit_code" >> "$log_file"
    fi
    echo >> "$err_log"
    echo >> "$log_file"
}

# Iterate over each command and execute with timeout
for controller in "${controllers[@]}"; do
    execute_command "$controller"
done
