//! Types describing data the runner emits

use paralegal_policy::paralegal_spdg::{Identifier, SPDGStats, SPDG};
use paralegal_policy::Context;
use serde::{Deserialize, Serialize};
use std::process::Child;
use std::sync;
use std::thread;
use std::time::{Duration, Instant};

use crate::input::{EvaluationConfig, Expectation};
use crate::run::Run;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
struct TimeMeasurement(u128);

impl From<Duration> for TimeMeasurement {
    fn from(value: Duration) -> Self {
        Self(value.as_micros())
    }
}

#[derive(Serialize, Deserialize)]
pub struct RunMeasurements {
    id: u32,
    experiment: String,
    run: String,
    policy: String,
    expectation: Expectation,
    adaptive_depth: bool,
    result: Option<bool>,
    pdg_time: TimeMeasurement,
    rustc_time: Option<TimeMeasurement>,
    policy_time: Option<TimeMeasurement>,
    deserialization_time: Option<TimeMeasurement>,
    precomputation_time: Option<TimeMeasurement>,
    traversal_time: Option<TimeMeasurement>,
    num_controllers: Option<u16>,
    peak_mem_usage_pdg: u64,
    mean_mem_usage_pdg: u64,
    peak_cpu_usage_pdg: f32,
    mean_cpu_usage_pdg: f32,
    peak_mem_usage_policy: Option<u64>,
    mean_mem_usage_policy: Option<u64>,
    peak_cpu_usage_policy: Option<f32>,
    mean_cpu_usage_policy: Option<f32>,
}

impl RunMeasurements {
    pub fn new(
        id: u32,
        experiment: String,
        run: String,
        policy: String,
        expectation: Expectation,
        adaptive_depth: bool,
        pdg_stat: CommandMeasurement,
    ) -> Self {
        Self {
            id,
            experiment,
            run,
            policy,
            expectation,
            result: None,
            pdg_time: pdg_stat.elapsed.into(),
            adaptive_depth,
            rustc_time: None,
            policy_time: None,
            deserialization_time: None,
            precomputation_time: None,
            traversal_time: None,
            num_controllers: None,
            peak_cpu_usage_pdg: pdg_stat.peak_cpu_usage,
            peak_cpu_usage_policy: None,
            mean_cpu_usage_pdg: pdg_stat.mean_cpu_usage,
            mean_cpu_usage_policy: None,
            peak_mem_usage_pdg: pdg_stat.peak_mem_usage,
            peak_mem_usage_policy: None,
            mean_mem_usage_pdg: pdg_stat.mean_mem_usage,
            mean_mem_usage_policy: None,
        }
    }

    pub fn from_experiment(id: u32, exp: &Run, pdg_stat: CommandMeasurement) -> Self {
        Self::new(
            id,
            exp.experiment_name.to_owned(),
            exp.name(),
            exp.policy_name.to_owned(),
            exp.expectation,
            exp.config.adaptive_depth,
            pdg_stat,
        )
    }

    pub fn add_policy_stat(
        &mut self,
        cmd_stat: CommandMeasurement,
        ctx: &Context,
        success: bool,
        traversal_time: Duration,
    ) {
        macro_rules! set {
            ($field:ident, $target:expr) => {
                assert!(self.$field.replace($target).is_none());
            };
        }
        set!(mean_cpu_usage_policy, cmd_stat.mean_cpu_usage);
        set!(peak_mem_usage_policy, cmd_stat.peak_mem_usage);
        set!(
            precomputation_time,
            ctx.context_stats().precomputation.into()
        );
        set!(result, success);
        set!(
            deserialization_time,
            ctx.context_stats().deserialization.unwrap().into()
        );
        set!(traversal_time, traversal_time.into());
        set!(num_controllers, ctx.desc().controllers.len() as u16);
        set!(rustc_time, ctx.desc().rustc_time.into());
    }
}

#[derive(Serialize, Deserialize)]
pub struct SystemParameters {
    num_cores: u16,
    num_physical_cores: u16,
    cpu_brand: String,
    cpu_frequency: u64,
    cpu_vendor_id: String,
    max_mem: u64,
    max_swap: u64,
    cpu_arch: Option<String>,
    kernel_version: Option<String>,
    os_version: Option<String>,
}

impl SystemParameters {
    pub fn new() -> Self {
        use sysinfo::System;
        let sys = System::new_all();
        let cpus = sys.cpus();
        let cpu = cpus.first().unwrap();
        let cpu_brand = cpu.brand().to_owned();
        let cpu_frequency = cpu.frequency();
        let cpu_vendor_id = cpu.vendor_id().to_owned();
        for cpu in cpus {
            assert_eq!(cpu_brand, cpu.brand());
            assert_eq!(cpu_frequency, cpu.frequency());
            assert_eq!(cpu_vendor_id, cpu.vendor_id());
        }
        Self {
            num_cores: cpus.len() as u16,
            num_physical_cores: sys.physical_core_count().unwrap() as u16,
            cpu_vendor_id,
            cpu_brand,
            cpu_frequency,
            max_mem: sys.total_memory(),
            max_swap: sys.total_swap(),
            cpu_arch: System::cpu_arch(),
            os_version: System::long_os_version(),
            kernel_version: System::kernel_version(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ControllerMeasurement {
    run_id: u32,
    name: Identifier,
    num_nodes: u32,
    max_inlining_depth: u16,
    mean_inlining_depth: f32,
    num_edges: u32,
    unique_locs: u32,
    unique_functions: u32,
    analyzed_locs: u32,
    analyzed_functions: u32,
    inlinings_performed: u32,
    construction_time: TimeMeasurement,
    conversion_time: TimeMeasurement,
}

impl ControllerMeasurement {
    pub fn from_spdg(run_id: u32, spdg: &SPDG) -> Self {
        let inlining_sum = spdg.graph.node_weights().map(|w| w.at.len()).sum::<usize>();
        let SPDGStats {
            unique_locs,
            unique_functions,
            analyzed_locs,
            analyzed_functions,
            inlinings_performed,
            construction_time,
            conversion_time,
        } = spdg.statistics.clone();
        Self {
            run_id,
            name: spdg.name,
            num_nodes: spdg.graph.node_count() as u32,
            max_inlining_depth: spdg.graph.node_weights().map(|w| w.at.len()).max().unwrap() as u16,
            mean_inlining_depth: inlining_sum as f32 / spdg.graph.node_count() as f32,
            num_edges: spdg.graph.edge_count() as u32,
            unique_locs,
            unique_functions,
            analyzed_locs,
            analyzed_functions,
            inlinings_performed,
            construction_time: construction_time.into(),
            conversion_time: conversion_time.into(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct CommandMeasurement {
    peak_cpu_usage: f32,
    mean_cpu_usage: f32,
    peak_mem_usage: u64,
    mean_mem_usage: u64,
    elapsed: TimeMeasurement,
}

impl CommandMeasurement {
    pub fn for_self<R>(config: &EvaluationConfig, f: impl FnOnce() -> R) -> (R, Self) {
        Self::collect(config, std::process::id(), f)
    }

    pub fn for_process(config: &EvaluationConfig, process: &mut Child) -> std::io::Result<Self> {
        let pid = process.id();
        let (_, stat) = Self::collect(config, pid, || process.wait().unwrap());

        Ok(stat)
    }

    fn collect<R>(config: &EvaluationConfig, pid: u32, f: impl FnOnce() -> R) -> (R, Self) {
        let mut sys_stat = sysinfo::System::new();
        let pid = sysinfo::Pid::from_u32(pid);
        let sync = sync::OnceLock::new();
        thread::scope(|scope| {
            let handle = scope.spawn(|| {
                let mut sum_mem = 1;
                let mut num_samples = 0;
                let mut sum_cpu = 0.0_f32;
                let mut peak_cpu = 0.0_f32;
                let mut peak_mem = 0;

                while sync.get().is_none() {
                    std::thread::sleep(config.stat_refresh_interval);
                    sys_stat.refresh_process(pid);
                    if let Some(proc_info) = sys_stat.process(pid) {
                        peak_mem = peak_mem.max(proc_info.memory());
                        sum_mem += proc_info.memory();
                        sum_cpu += proc_info.cpu_usage();
                        peak_cpu = peak_cpu.max(proc_info.cpu_usage());
                        num_samples += 1;
                    }
                }

                CommandMeasurement {
                    peak_cpu_usage: peak_cpu,
                    peak_mem_usage: peak_mem,
                    mean_cpu_usage: sum_cpu / num_samples.max(1) as f32,
                    mean_mem_usage: sum_mem / num_samples.max(1),
                    elapsed: Duration::ZERO.into(),
                }
            });
            let start = Instant::now();
            let result = f();
            let elapsed = start.elapsed();
            sync.set(()).unwrap();
            let mut stats = handle.join().unwrap();
            stats.elapsed = elapsed.into();
            (result, stats)
        })
    }
}
