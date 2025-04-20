#!/usr/bin/env python3
import json
import argparse

def print_a_stat(stat, args):
    indent = '   '
    has_var_markers = any(len(v['markers']) != 0 for v in stat['markers_from_variables'])
    has_reachable_markers = len(stat['calls_with_reachable_markers']) != 0
    has_markers_on_self = len(stat['markers_on_self']) != 0

    if args.select is not None\
        and len(args.select) != 0\
        and i['function']['ident'] not in args.select:
        return
    if args.root_marked and not (has_var_markers or has_reachable_markers or has_markers_on_self):
        return
    # if not (has_var_markers or has_reachable_markers or has_markers_on_self):
    #     return
    
    # if not (has_var_markers or has_markers_on_self):
    #     return
    print(format_function(stat['function']))
    if stat['is_constructor']:
        print(indent, 'is construtor')
    is_async = stat['is_async']
    if is_async is not None:
        print(indent, 'async closure', is_async)
    is_stubbed = stat['is_stubbed']
    if is_stubbed is not None:
        print(indent, 'stubbed by', is_stubbed)
    if has_var_markers:
        print(indent, 'markers from variables:')
        for v in stat['markers_from_variables']:
            if len(v['markers']) == 0:
                continue
            print(indent, indent, v['local'], truncate_string(v['type']), v['markers'])
    if has_markers_on_self:
        print(indent, 'markers on self:', stat['markers_on_self'])
    if has_reachable_markers:
        print(indent, 'calls with reachable markers:')
        for call in stat['calls_with_reachable_markers']:
            print(indent, indent, format_function(call['function']), call['span'])

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("file")
    parser.add_argument('--select', '-s', action='append', help='Select a specific function to print')
    parser.add_argument('--root-marked', action='store_true', help='Only show functions that are marked directly')

    args = parser.parse_args()

    with open(args.file, "r") as f:
        data = json.load(f)

    print("Found", len(data), "functions")
    for i in data:
        print_a_stat(i, args)
    return

    filtered = [
        stat for stat in data
        if any(len(v['markers']) != 0 for v in stat['markers_from_variables']) or len(stat['calls_with_reachable_markers']) != 0 or len(stat['markers_on_self']) != 0
    ]   
    print(f"Found {len(filtered)} functions with markers out of {len(data)}")
    print_stats(function_histogram(filtered))
    print_causes(filtered)

    # for stat in data:
    #     print_a_stat(stat)

def print_causes(data):
    self_marked = 0
    var_marked = 0
    reachable_marked = 0
    for stat in data:
        if len(stat['markers_on_self']) != 0:
            self_marked += 1
        if any(len(v['markers']) != 0 for v in stat['markers_from_variables']):
            var_marked += 1
        if len(stat['calls_with_reachable_markers']) != 0:
            reachable_marked += 1
    print(f"self marked: {self_marked}")
    print(f"var marked: {var_marked}")
    print(f"reachable marked: {reachable_marked}")

def format_function(function):
    return function['ident'] + ' ' + str(truncate_args(function['args']))

def truncate_args(args):
    if args is None:
        return args
    if len(args) < 4:
        return args
    else:
        return args[:3] + [f'+ {len(args) - 3} more']

def truncate_string(s):
    if len(s) < 30:
        return s
    else:
        return s[:27] + '+' + str(len(s) - 27) + ' more'

def function_histogram(data):
    histogram = {}
    for stat in data:
        function = stat['function']['ident']
        if function not in histogram:
            histogram[function] = 0
        histogram[function] += 1
    return histogram

def print_stats(complete_histogram):
    histogram = sorted(complete_histogram.items(), key=lambda x: x[1], reverse=True)
    teaser = histogram[:10]
    max_key_length = max(len(key) for key, _ in teaser)
    max_value = max(val for _, val in teaser)
    scale = 50 / max_value if max_value > 50 else 1

    for key, value in teaser:
        bar = '#' * int(value * scale)
        print(f"{key.ljust(max_key_length)} | {bar} ({value})")
    
    #print("This accounts for", sum(val for _, val in histogram) / sum(complete_histogram.values()) * 100, "% of the total functions")

    
    quantile_thresholds = [0.25, 0.5, 0.75, 1]
    quantile_threshold_iter = iter(quantile_thresholds)
    quantiles = []
    total_calls = sum(complete_histogram.values())
    agg = 0
    funs = 0
    avg = 0
    quantile_threshold = next(quantile_threshold_iter)
    for _, val in histogram:
        agg += val
        avg += val
        funs += 1
        if quantile_threshold is None:
            pass
        if agg / total_calls >= quantile_threshold:
            quantiles.append((funs, avg / funs))
            avg = 0
            funs = 0
            try:
                quantile_threshold = next(quantile_threshold_iter)
            except StopIteration:
                quantile_threshold = None
    print("Quantiles:")
    for quantile, (funs, avg) in zip(quantile_thresholds, quantiles):
        print(f"  {quantile:<4}: {funs:>4} functions, {avg:.2f} avg calls")




if __name__ == "__main__":
    main()