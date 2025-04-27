use allocative::Allocative;
use bincode;
use clap::Parser;
use paralegal_policy::paralegal_spdg::{
    allocative_visit_map_coerce_key,
    utils::{serde_map_via_vec, TruncatedHumanTime},
    CallString, DefKind, DisplayPath,
};
use paralegal_policy::ProgramDescription;
use serde::Serialize;
use stats_alloc::{StatsAlloc, INSTRUMENTED_SYSTEM};
use std::io::Write;
use std::path::PathBuf;
use std::{alloc::System, collections::HashMap};
use std::{convert::identity, fmt};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

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

fn get_allocated_memory_sysinfo() -> usize {
    let mut system = sysinfo::System::new_all();
    system.refresh_memory();
    system.used_memory() as usize
}

fn get_allocated_memory_stats_alloc() -> usize {
    let stats = GLOBAL.stats();
    stats.bytes_allocated - stats.bytes_deallocated
}

fn top_k_by_key<T, E: Ord, I: Iterator<Item = T>, F: Fn(&T) -> E>(
    iter: I,
    k: usize,
    f: F,
) -> Vec<T> {
    let mut heap = Vec::with_capacity(k);
    if k == 0 {
        return heap;
    }
    let sorted_insert = |heap: &mut Vec<T>, item: T| {
        let as_ref = &f(&item);
        let idx = heap
            .binary_search_by(|a| as_ref.cmp(&f(a)))
            .map_or_else(identity, identity);
        heap.insert(idx, item);
    };
    for item in iter {
        if heap.len() < k {
            sorted_insert(&mut heap, item);
        } else {
            let last = heap.pop().unwrap();
            if f(&item) > f(&last) {
                sorted_insert(&mut heap, item);
            } else {
                heap.push(last);
            }
        }
    }
    heap
}

fn max_and_avg(iter: impl Iterator<Item = usize>) -> (usize, usize) {
    let mut max = 0;
    let mut sum = 0;
    let mut count = 0;

    for item in iter {
        if item > max {
            max = item;
        }
        sum += item;
        count += 1;
    }

    let avg = if count > 0 { sum / count } else { 0 };
    (max, avg)
}

fn print_controller_stat<T>(
    stats: Vec<T>,
    name_fn: impl Fn(&T) -> &str,
    value_fn: impl Fn(&T) -> usize,
) {
    let max_key_size = stats.iter().map(|s| name_fn(s).len()).max().unwrap_or(0);
    let max_value_size = stats
        .iter()
        .map(|s| value_fn(s).to_string().len())
        .max()
        .unwrap_or(0);

    for stat in stats {
        println!(
            "{:<max_key_size$} : {:>max_value_size$}",
            name_fn(&stat),
            HumanInt(value_fn(&stat))
        );
    }
}

fn main() {
    let args = Arguments::parse();

    let mem_before_load = get_allocated_memory_sysinfo();
    println!(
        "Memory before load (as per sysinfo): {}",
        HumanBytes(mem_before_load)
    );
    println!(
        "Memory before load (as per stats_alloc): {}",
        HumanBytes(get_allocated_memory_stats_alloc())
    );

    let loading_start = std::time::Instant::now();
    let graph = ProgramDescription::canonical_read(&args.path).unwrap();
    let loading_duration = loading_start.elapsed();

    let mut flame_graph_builder = allocative::FlameGraphBuilder::default();
    flame_graph_builder.visit_root(&graph);
    let out = flame_graph_builder.finish();

    let (max_call_string_len, avg_call_string_len) = max_and_avg(
        graph
            .controllers
            .values()
            .flat_map(|v| v.graph.node_weights().map(|n| n.at.len())),
    );

    // let num_samples = 3;
    // let mut max_call_string_samples = vec![];
    // let mut avg_call_string_samples = vec![];

    // for cs in graph
    //     .controllers
    //     .values()
    //     .flat_map(|v| v.graph.node_weights())
    //     .map(|n| n.at)
    // {
    //     if cs.len() == max_call_string_len && max_call_string_samples.len() < num_samples {
    //         max_call_string_samples.push(cs.clone());
    //     }
    //     if cs.len() == avg_call_string_len && avg_call_string_samples.len() < num_samples {
    //         avg_call_string_samples.push(cs.clone());
    //     }
    // }

    let table = [
        (
            "Load Time",
            &TruncatedHumanTime::from(loading_duration) as &dyn std::fmt::Display,
        ),
        (
            "Total program memory (stats_alloc)",
            &HumanBytes(get_allocated_memory_stats_alloc()),
        ),
        (
            "Total program memory (sysinfo)",
            &HumanBytes(get_allocated_memory_sysinfo()),
        ),
        (
            "Graph size in memory (allocative)",
            &HumanBytes(out.flamegraph().total_size()),
        ),
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
        ("Max Call String Length", &HumanInt(max_call_string_len)),
        ("Average Call String Length", &HumanInt(avg_call_string_len)),
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

    let controller_stats = graph
        .controllers
        .iter()
        .map(|(k, v)| {
            let num_nodes = v.graph.node_count();
            let num_edges = v.graph.edge_count();
            let (max_call_string_len, avg_call_string_len) =
                max_and_avg(v.graph.node_weights().map(|n| n.at.len()));
            (
                format!("{}", DisplayPath::from(&graph.def_info[k].path)),
                num_nodes,
                num_edges,
                max_call_string_len,
                avg_call_string_len,
            )
        })
        .collect::<Vec<_>>();

    let most_nodes_controllers =
        top_k_by_key(controller_stats.iter(), 5, |(_, num_nodes, _, _, _)| {
            *num_nodes
        });
    let most_edges_controllers =
        top_k_by_key(controller_stats.iter(), 5, |(_, _, num_edges, _, _)| {
            *num_edges
        });
    let largest_call_strings_controllers = top_k_by_key(
        controller_stats.iter(),
        5,
        |(_, _, _, max_call_string_len, _)| *max_call_string_len,
    );
    let largest_avg_call_strings_controllers = top_k_by_key(
        controller_stats.iter(),
        5,
        |(_, _, _, _, avg_call_string_len)| *avg_call_string_len,
    );

    println!("Controllers with most nodes:");

    print_controller_stat(
        most_nodes_controllers,
        |(name, _, _, _, _)| name,
        |(_, num_nodes, _, _, _)| *num_nodes,
    );
    println!("");

    println!("Controllers with most edges:");
    print_controller_stat(
        most_edges_controllers,
        |(name, _, _, _, _)| name,
        |(_, _, num_edges, _, _)| *num_edges,
    );
    println!("");

    println!("Controllers with largest call strings:");
    print_controller_stat(
        largest_call_strings_controllers,
        |(name, _, _, _, _)| name,
        |(_, _, _, max_call_string_len, _)| *max_call_string_len,
    );
    println!("");

    println!("Controllers with largest average call strings:");
    print_controller_stat(
        largest_avg_call_strings_controllers,
        |(name, _, _, _, _)| name,
        |(_, _, _, _, avg_call_string_len)| *avg_call_string_len,
    );
    println!("");

    fn print_call_string(cs: CallString, graph: &ProgramDescription) {
        let leaf_instruction = cs.leaf();
        let leaf_instruction_info = &graph.instruction_info[&leaf_instruction];
        println!("    {}", leaf_instruction_info.description);
        println!(
            "        in {}:{}",
            leaf_instruction_info.span.source_file.file_path, leaf_instruction_info.span.start.line
        );

        for loc in cs.iter() {
            let fun_info = &graph.def_info[&loc.function];
            let instruction_info = &graph.instruction_info[&loc];
            print!("    {}", DisplayPath::from(&fun_info.path));
            if fun_info.kind == DefKind::Fn {
                println!("");
            } else {
                println!(" ({})", fun_info.kind.as_ref());
            }
            let span = &instruction_info.span;
            println!(
                "        in {}:{}",
                span.source_file.file_path, span.start.line
            );
        }
    }

    // println!("Sample maximal call strings:");
    // println!("");

    // for cs in max_call_string_samples {
    //     print_call_string(cs, &graph);
    //     println!("");
    // }
    // println!("");
    // println!("");

    // println!("Sample average call strings:");
    // println!("");

    // for cs in avg_call_string_samples {
    //     print_call_string(cs, &graph);
    //     println!("");
    // }

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

#[derive(Allocative, serde::Serialize)]
#[serde(bound = "K: serde::Serialize, V: serde::Serialize")]
#[allocative(bound = "'a, K, V: Allocative")]
struct SerdeMapViaVecAllocCoerce<'a, K, V>(
    #[serde(with = "serde_map_via_vec")]
    #[allocative(visit = allocative_visit_map_coerce_key)]
    &'a HashMap<K, V>,
);

fn compare_serialized_size() {
    #[derive(serde::Serialize, serde::Deserialize, Allocative, Clone)]
    struct ExampleStruct {
        int: i32,
        float: f64,
        string: String,
        optional: Option<Box<ExampleStruct>>,
    }

    #[derive(serde::Serialize, serde::Deserialize, Allocative, Clone)]
    enum ExampleEnum {
        Variant1(i32),
        Variant2(String),
        Variant3(Box<ExampleStruct>),
    }

    let example = ExampleStruct {
        int: 42,
        float: 3.14,
        string: "Hello, world!".to_string(),
        optional: Some(Box::new(ExampleStruct {
            int: 7,
            float: 2.71,
            string: "Nested struct".to_string(),
            optional: Some(Box::new(ExampleStruct {
                int: 99,
                float: 1.23,
                string: "Deeper nested struct".to_string(),
                optional: None,
            })),
        })),
    };

    let example_enum = ExampleEnum::Variant3(Box::new(example.clone()));

    report("ExampleStruct", &example);
    report("ExampleEnum", &example_enum);
    report("Box<ExampleStruct>", &Box::new(example.clone()));
    report(
        "Deeply Nested Struct",
        &ExampleStruct {
            int: 1,
            float: 0.1,
            string: "Deeply nested".to_string(),
            optional: Some(Box::new(ExampleStruct {
                int: 2,
                float: 0.2,
                string: "Level 2".to_string(),
                optional: Some(Box::new(ExampleStruct {
                    int: 3,
                    float: 0.3,
                    string: "Level 3".to_string(),
                    optional: Some(Box::new(ExampleStruct {
                        int: 4,
                        float: 0.4,
                        string: "Level 4".to_string(),
                        optional: None,
                    })),
                })),
            })),
        },
    );
}
fn report(name: &str, obj: &(impl Allocative + Serialize)) {
    let serialized = bincode::serialize(obj).unwrap();
    let serialized_size = serialized.len();
    let original_size = {
        let mut builder = allocative::FlameGraphBuilder::default();
        builder.visit_root(obj);
        builder.finish().flamegraph().total_size()
    };

    println!(
        "{}: Original size = {}, Serialized size = {}",
        name,
        HumanBytes(original_size),
        HumanBytes(serialized_size)
    );
}
