//! Types describing data the runner emits

use paralegal_policy::paralegal_spdg::{Identifier, SPDGStats, SPDG};
use paralegal_policy::Context;
use serde::{Deserialize, Serialize};
use std::process::Child;
use std::sync;
use std::thread;
use std::time::{Duration, Instant};

use crate::input::{EvaluationConfig, PolicyResult};
use crate::run::Run;
use crate::GRISWOLD_COMMIT;

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
    mode: String,
    application: String,
    /// If only one controller is selected this is set. (Usually lemmy)
    controller: Option<String>,
    /// The features that selects the ablation configuration. Only set in ablation experiments.
    ablation_feature: Option<String>,
    /// Only set in roll-forward experiments. Indicates the commit this was run on.
    commit: Option<String>,
    /// Used by lemmy to identify the bug that was tested
    bug: Option<String>,
    run: String,
    policy: String,
    expectation: PolicyResult,
    adaptive_depth: bool,
    result: Option<PolicyResult>,
    /// Total time takes by the `cargo paralegal-flow` command
    pdg_time: TimeMeasurement,
    pdg_timed_out: bool,
    rustc_time: Option<TimeMeasurement>,
    /// Total time spent executing the policy
    policy_time: Option<TimeMeasurement>,
    deserialization_time: Option<TimeMeasurement>,
    /// Time spent preparing the `Context`
    precomputation_time: Option<TimeMeasurement>,
    /// Time spent in graph queries
    traversal_time: Option<TimeMeasurement>,
    num_controllers: Option<u16>,
    /// How many PDG nodes have markers assigned.
    num_markers: Option<u32>,
    dedup_functions: Option<u32>,
    dedup_locs: Option<u32>,
    /// How many of the analyzed lines changed vs the previous commit. Used in
    /// roll-forward only
    changed_lines: Option<u32>,
    /// Size of the SPDG file in bytes
    file_size: Option<u64>,
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
    pub fn from_experiment(id: u32, exp: &Run, pdg_stat: CommandMeasurement) -> Self {
        Self {
            id,
            experiment: exp.experiment_name.to_owned(),
            application: exp.config.app_config_name().to_owned(),
            mode: exp.config.mode.as_ref().to_owned(),
            run: exp.name(),
            policy: exp.policy_name.to_owned(),
            expectation: exp.expectation,
            controller: exp.controller.map(ToOwned::to_owned),
            ablation_feature: exp.ablation_feature.map(ToOwned::to_owned),
            commit: exp.commit.clone(),
            bug: exp.bug.map(ToOwned::to_owned),
            result: None,
            pdg_time: pdg_stat.elapsed.into(),
            adaptive_depth: exp.config.adaptive_depth,
            rustc_time: None,
            policy_time: None,
            pdg_timed_out: pdg_stat.timed_out,
            deserialization_time: None,
            precomputation_time: None,
            traversal_time: None,
            num_controllers: None,
            changed_lines: None,
            num_markers: None,
            dedup_functions: None,
            dedup_locs: None,
            file_size: None,
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

    pub fn add_policy_stat(
        &mut self,
        cmd_stat: CommandMeasurement,
        ctx: &Context,
        success: PolicyResult,
        traversal_time: Duration,
        file_size: u64,
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
        set!(policy_time, cmd_stat.elapsed);
        set!(num_markers, ctx.desc().marker_annotation_count);
        set!(dedup_functions, ctx.desc().dedup_functions);
        set!(dedup_locs, ctx.desc().dedup_locs);
        set!(file_size, file_size);
    }

    pub fn add_changed_lines(&mut self, l: u32) {
        assert!(self.changed_lines.replace(l).is_none())
    }
}

#[derive(Serialize)]
pub struct SystemParameters {
    num_physical_cores: u16,
    cpus: Box<[CpuParameters]>,
    max_mem: u64,
    max_swap: u64,
    cpu_arch: Option<String>,
    kernel_version: Option<String>,
    os_version: Option<String>,
    paralegal_commit: String,
    griswold_commit: String,
    repo_commit: String,
}

#[derive(Serialize)]
struct CpuParameters {
    brand: String,
    frequency: u64,
    vendor_id: String,
}

impl SystemParameters {
    pub fn new(paralegal_commit: String, repo_commit: String) -> Self {
        use sysinfo::System;
        let sys = System::new_all();
        let cpus = sys
            .cpus()
            .iter()
            .map(|cpu| CpuParameters {
                brand: cpu.brand().to_owned(),
                frequency: cpu.frequency(),
                vendor_id: cpu.vendor_id().to_owned(),
            })
            .collect();

        Self {
            num_physical_cores: sys.physical_core_count().unwrap() as u16,
            cpus,
            max_mem: sys.total_memory(),
            max_swap: sys.total_swap(),
            cpu_arch: System::cpu_arch(),
            os_version: System::long_os_version(),
            kernel_version: System::kernel_version(),
            paralegal_commit,
            griswold_commit: GRISWOLD_COMMIT.to_owned(),
            repo_commit,
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
    timed_out: bool,
}

struct CommandMeasurementCollector {
    sum_mem: u64,
    num_samples: u64,
    sum_cpu: f32,
    peak_cpu: f32,
    peak_mem: u64,
    sys_stat: sysinfo::System,
    pid: sysinfo::Pid,
    start: Instant,
}

impl CommandMeasurementCollector {
    fn new(pid: sysinfo::Pid) -> Self {
        Self {
            sum_mem: 0,
            num_samples: 0,
            sum_cpu: 0.0,
            peak_cpu: 0.0,
            peak_mem: 0,
            sys_stat: sysinfo::System::new(),
            pid,
            start: Instant::now(),
        }
    }

    fn tick(&mut self) {
        if let Some(proc_info) = self.sys_stat.process(self.pid) {
            self.peak_mem = self.peak_mem.max(proc_info.memory());
            self.sum_mem += proc_info.memory();
            self.sum_cpu += proc_info.cpu_usage();
            self.peak_cpu = self.peak_cpu.max(proc_info.cpu_usage());
            self.num_samples += 1;
        }
    }
}

impl CommandMeasurementCollector {
    fn into_measurement(self, timed_out: bool) -> CommandMeasurement {
        CommandMeasurement {
            peak_cpu_usage: self.peak_cpu,
            peak_mem_usage: self.peak_mem,
            mean_cpu_usage: self.sum_cpu / self.num_samples.max(1) as f32,
            mean_mem_usage: self.sum_mem / self.num_samples.max(1),
            elapsed: self.start.elapsed().into(),
            timed_out,
        }
    }
}

impl CommandMeasurement {
    pub fn for_self<R>(config: &EvaluationConfig, f: impl FnOnce() -> R) -> (R, Self) {
        let sync = sync::OnceLock::new();
        thread::scope(|scope| {
            let handle = scope.spawn(|| {
                let mut collector =
                    CommandMeasurementCollector::new(sysinfo::Pid::from_u32(std::process::id()));
                while sync.get().is_none() {
                    std::thread::sleep(config.stat_refresh_interval);
                    collector.tick();
                }
                collector.into_measurement(false)
            });
            let start = Instant::now();
            let result = f();
            let elapsed = start.elapsed();
            sync.set(()).unwrap();
            let mut stat: Self = handle.join().unwrap();
            stat.elapsed = elapsed.into();
            (result, stat)
        })
    }

    pub fn for_process(
        config: &EvaluationConfig,
        timeout: Option<Duration>,
        process: &mut Child,
    ) -> std::io::Result<Self> {
        let pid = process.id();

        let mut collector = CommandMeasurementCollector::new(sysinfo::Pid::from_u32(pid));
        let mut timed_out = false;

        while process.try_wait()?.is_none() {
            std::thread::sleep(config.stat_refresh_interval);
            collector.tick();
            if let Some(timeout) = timeout {
                if timeout < collector.start.elapsed() {
                    process.kill()?;
                    timed_out = true;
                    break;
                }
            }
        }

        let stat = collector.into_measurement(timed_out);

        Ok(stat)
    }
}
