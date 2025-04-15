use clap::Parser;
use paralegal_policy::paralegal_spdg::rustc_proxies::{CrateNum, Location};
use paralegal_policy::paralegal_spdg::utils::TruncatedHumanTime;
use paralegal_policy::paralegal_spdg::{CallString, GlobalLocation, Identifier};
use paralegal_policy::{paralegal_spdg, DefId, GraphLocation, ProgramDescription};
use std::fmt;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser)]
struct Arguments {
    path: PathBuf,
}

struct HumanBytes(usize);

impl fmt::Display for HumanBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let size = self.0 as f64;
        const KB: f64 = 1024.0;
        const MB: f64 = KB * 1024.0;
        const GB: f64 = MB * 1024.0;

        if size >= GB {
            write!(f, "{:.2} GB", size / GB)
        } else if size >= MB {
            write!(f, "{:.2} MB", size / MB)
        } else if size >= KB {
            write!(f, "{:.2} KB", size / KB)
        } else {
            write!(f, "{} bytes", self.0)
        }
    }
}

struct HumanInt(usize);

impl fmt::Display for HumanInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let num = self.0;
        let mut as_str = num.to_string();
        let len = as_str.len();
        for i in (3..len).step_by(3) {
            as_str.insert(len - i, ',');
        }

        f.write_str(&as_str)
    }
}

macro_rules! show_size {
    ($e:expr) => {
        println!("Size of {}: {}", stringify!($e), allocative::size_of_unique($e));
    }
}

fn main() {
    let args = Arguments::parse();

    let loading_start = std::time::Instant::now();
    let graph = ProgramDescription::canonical_read(&args.path).unwrap();
    let loading_duration = loading_start.elapsed();

    let mut flame_graph_builder = allocative::FlameGraphBuilder::default();
    flame_graph_builder.visit_root(&graph);
    let out = flame_graph_builder.finish();

    let table = [
        (
            "Load Time",
            &TruncatedHumanTime::from(loading_duration) as &dyn std::fmt::Display,
        ),
        ("Size in memory", &HumanBytes(out.flamegraph().total_size())),
        ("PDGs", &graph.controllers.len() as &_),
        (
            "Size on disk",
            &HumanBytes(
                std::fs::File::open(&args.path)
                    .unwrap()
                    .metadata()
                    .unwrap()
                    .len() as _,
            ) as &_,
        ),
        (
            "Total nodes",
            &HumanInt(
                graph
                    .controllers
                    .values()
                    .map(|pdg| pdg.graph.node_count())
                    .sum::<usize>(),
            ) as &_,
        ),
        (
            "Total edges",
            &HumanInt(
                graph
                    .controllers
                    .values()
                    .map(|pdg| pdg.graph.edge_count())
                    .sum::<usize>(),
            ) as &_,
        ),
    ];
    let table = table
        .iter()
        .map(|(k, v)| (*k, v.to_string()))
        .collect::<Vec<_>>();

    let max_key_size = table.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    let max_value_size = table.iter().map(|(_, v)| v.len()).max().unwrap_or(0);

    for (k, v) in table.iter() {
        println!("{:<max_key_size$} : {:>max_value_size$}", k, v);
    }

    let flamegraph_file = std::path::Path::new("flamegraph.svg");
    let mut proc = std::process::Command::new("inferno-flamegraph")
        .stdin(std::process::Stdio::piped())
        .stdout(std::fs::File::create(flamegraph_file).unwrap())
        .spawn()
        .unwrap();
    let mut stdin = proc.stdin.take().unwrap();
    stdin
        .write_all(out.flamegraph().write().as_bytes())
        .unwrap();
    drop(stdin);
    let status = proc.wait().unwrap();
    if !status.success() {
        panic!("inferno-flamegraph failed");
    }
}
