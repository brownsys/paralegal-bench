# Case Studies and Experiment Runner for the Paralegal Static Analyzer

This repository contains:

## Case studies

The `case-studies` directory contains the source code for a collection of
applications, frozen in time and sometimes partially altered, for which we
formalized policies and assigned markers.

## Policies

The `policies` directory contains implementations for policies corresponding to
the case study applications.

Each policy comprises of a `lib` portion that defines the policy, as well as a
standalone application, named after the case study, that enforces the policy on
the case study code in `case-studies`.

To run such an individual policy use `cargo run --release --bin
<case-study-name>`. The policies have command line arguments which you can
explore by passing `--help` like so: `cargo run --release --bin
<case-study-name> -- --help`.

## The Benchmarker

`griswold` is our push-button benchmarker that runs various configuration of the
policies on the case studies. It reasons about expected and actual outcomes
while collecting performance metrics like runtime, graph size and lines of code
analyzed.

Each benchmark run is controlled by a central TOML file. The format of the file
is defined by and documented as the data structures in
[input.rs](griswold/src/input.rs). Examples of the bench configurations are
found in the `bconf` directory.

It's results are timestamped and written to the `results` directory. The output
format is defined by the data structures in [output.rs](griswold/src/output.rs).

For additional information see [the Notion page](https://www.notion.so/justus-adam/Documentation-The-griswold-benchmark-runner-5441bb26f75d4fc4a37cc613ab3a65c6?pvs=4).

## Misc

- `roll-forward` contains additional files pertaining to the roll forward
  experiment for AtomicData and Plume