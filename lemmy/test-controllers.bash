#!/bin/bash

# Thx chatgpt for the raw version of this

controllers=(
    "comment-like" 
    "community-add-mod" 
    "community-ban"
    "community-hide"
    "user-ban-person"
    "post-like"
    "post-lock"
    "post-sticky"
    "comment-create"
    "comment-delete"
    "comment-remove"
	"community-remove"
	"community-update"
	"post-create"
	"post-delete"
	"post-remove"
	"post-update"
    "private-message-delete"
)

# Log file path
log_file="log.txt"
err_log="err.txt"

# Function to execute commands with timeout
execute_command() {
    local controller=$1
    timeout 5m cargo paralegal-flow --abort-after-analysis --target lemmy_api --verbose -- --features $controller >> "$log_file" 2>> "$err_log"
    local exit_code=$?
    if [ $exit_code -eq 124 ]; then
        echo "$(date) - Timeout reached for command: $controller" >> "$log_file"
    elif [ $exit_code -eq 137 ]; then
        echo "$(date) - Analysis for '$controller' killed by the operating system due to out-of-memory." >> "$log_file"
    else
        echo "$(date) - Exit code for controller '$controller': $exit_code" >> "$log_file"
    fi
}

# Iterate over each command and execute with timeout
for controller in "${controllers[@]}"; do
    execute_command "$controller"
done