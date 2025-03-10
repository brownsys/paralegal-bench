import json
import statistics
from collections import Counter

HEADER = """
Items are in-degrees, frequency shown is how many functions have such an
in-degree.

Results are a lower bound as they do not include compiler libs (std, core, libc)
and the traversal seems to be unable to execute certain monomorphizations and
some functions I was unable to dump MIR for. 
"""

def ascii_histogram(data):
    data = [
        v for v in data if v != 0
    ]
    frequency = Counter(data)
    max_count = max(frequency.values())
    
    for key in sorted(frequency.keys()):
        count = frequency[key]
        bar = "#" * (count * 50 // max_count)  # Scale to fit within 50 characters
        print(f"{key:3} | {bar} ({count})")
    
    mean = statistics.mean(data)
    stdev = statistics.stdev(data) if len(data) > 1 else 0
    print(f"\nAverage: \t\t{mean:.2f}")
    print(f"Standard Deviation: \t{stdev:.2f}")
    print(f"Total Function: \t{len(data)}")
    print(f"Total Calls: \t\t{sum(data)}")

if __name__ == "__main__":
    with open("inlining-stats.json", "r") as file:
        sample_data = json.load(file)

    print(HEADER)
    print()
    print()

    print("------ Histogram for all functions ------")
    data = [
        sum((stat['calls'] for stat in v.values()))
        for v in sample_data.values()
    ]
    ascii_histogram(data)

    print()
    print()

    print("------ Histogram for inlined functions ------")
    data = [
        v['inlined']['calls']
        for v in sample_data.values()
    ]
    ascii_histogram(data)

    print()
    print()

    inlined = sum((
        stat['inlined']['calls']
        for stat in sample_data.values()
    ))

    elided = sum((
        stat['elided']['calls']
        for stat in sample_data.values()
    ))

    library = sum((
        stat['library']['calls']
        for stat in sample_data.values()
    ))

    print("Function calls when considering:")
    print("Inlined only:\t", inlined)
    print("With elided:\t", inlined + elided)
    print("With library:\t", inlined + elided + library)