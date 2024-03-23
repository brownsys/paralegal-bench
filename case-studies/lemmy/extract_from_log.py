#!/usr/bin/env python3

def convert_to_seconds(time):
    """
    Convert time in various formats to seconds.
    """
    unit = time[-1]
    value = float(time[:-1])

    if unit == 'ns':
        seconds = value / 1e9
    elif unit == 'us':
        seconds = value / 1e6
    elif unit == 'ms':
        seconds = value / 1e3
    elif unit == 's':
        seconds = value
    elif unit == 'm':
        seconds = value * 60
    elif unit == 'h':
        seconds = value * 3600
    elif unit == 'd':
        seconds = value * 86400
    else:
        raise ValueError(f"Unknown time unit: {unit}")

    return seconds

# Initialize variables for min time and corresponding entry
min_time = float('inf')
min_entry = None

# Read each line of the table
with open('lemmy-log.txt', 'r') as f:
    for line in f:
        if 'atime' in line:
            continue

        parts = line.strip().split('|')
        if len(parts) < 5:
            continue
        controller = parts[0].strip()
        atime = parts[2].strip()
        conforms = parts[4].strip()

        # Check if conforms is unchecked and extract time in seconds
        if conforms == '❌':
            seconds = convert_to_seconds(atime)

            # If min_time is not set or current time is smaller, update min_time and min_entry
            if seconds < min_time:
                min_time = seconds
                min_entry = controller

# Output the entry with the lowest atime
if min_entry:
    print(f"Entry with lowest atime and unchecked conforms: {min_entry} with {min_time}s")
else:
    print("No entry found with unchecked conforms.")