//! Types describing data the runner emits

use paralegal_policy::paralegal_spdg::{Identifier, SPDGStats, SPDG};
use paralegal_policy::Context;
use serde::{Deserialize, Serialize};
use std::process::Child;
use std::sync;
use std::thread;
use std::time::{Duration, Instant};

use crate::input::Config;
use crate::run::Experiment;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
struct TimeMeasurement(u128);

impl From<Duration> for TimeMeasurement {
    fn from(value: Duration) -> Self {
        Self(value.as_micros())
    }
}

#[derive(Serialize, Deserialize)]
pub struct RunStat {
    id: u32,
    experiment: String,
    policy: String,
    expectation: bool,
    result: Option<bool>,
    pdg_time: TimeMeasurement,
    rustc_time: Option<TimeMeasurement>,
    policy_time: Option<TimeMeasurement>,
    deserialization_time: Option<TimeMeasurement>,
    precomputation_time: Option<TimeMeasurement>,
    traversal_time: Option<TimeMeasurement>,
    num_controllers: Option<u16>,
    peak_mem_usage_pdg: u64,
    avg_mem_usage_pdg: u64,
    peak_cpu_usage_pdg: f32,
    avg_cpu_usage_pdg: f32,
    peak_mem_usage_policy: Option<u64>,
    avg_mem_usage_policy: Option<u64>,
    peak_cpu_usage_policy: Option<f32>,
    avg_cpu_usage_policy: Option<f32>,
}

impl RunStat {
    pub fn new(
        id: u32,
        experiment: String,
        policy: String,
        expectation: bool,
        pdg_stat: CmdStat,
    ) -> Self {
        Self {
            id,
            experiment,
            policy,
            expectation,
            result: None,
            pdg_time: pdg_stat.elapsed.into(),
            rustc_time: None,
            policy_time: None,
            deserialization_time: None,
            precomputation_time: None,
            traversal_time: None,
            num_controllers: None,
            peak_cpu_usage_pdg: pdg_stat.peak_cpu,
            peak_cpu_usage_policy: None,
            avg_cpu_usage_pdg: pdg_stat.avg_cpu,
            avg_cpu_usage_policy: None,
            peak_mem_usage_pdg: pdg_stat.peak_mem,
            peak_mem_usage_policy: None,
            avg_mem_usage_pdg: pdg_stat.avg_mem,
            avg_mem_usage_policy: None,
        }
    }

    pub fn from_experiment(id: u32, exp: &Experiment, pdg_stat: CmdStat) -> Self {
        Self::new(
            id,
            exp.name(),
            exp.policy_name.to_owned(),
            exp.expectation,
            pdg_stat,
        )
    }

    pub fn add_policy_stat(
        &mut self,
        cmd_stat: CmdStat,
        ctx: &Context,
        success: bool,
        traversal_time: Duration,
    ) {
        macro_rules! set {
            ($field:ident, $target:expr) => {
                assert!(self.$field.replace($target).is_none());
            };
        }
        set!(avg_cpu_usage_policy, cmd_stat.avg_cpu);
        set!(peak_mem_usage_policy, cmd_stat.peak_mem);
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
pub struct SysStat {
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

impl SysStat {
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
pub struct ControllerStat {
    run_id: u32,
    name: Identifier,
    num_nodes: u32,
    #[serde(flatten)]
    statistics: SPDGStats,
    max_inlining_depth: u16,
    avg_inlining_depth: f32,
    num_edges: u32,
}

impl ControllerStat {
    pub fn from_spdg(run_id: u32, spdg: &SPDG) -> Self {
        let inlining_sum = spdg.graph.node_weights().map(|w| w.at.len()).sum::<usize>();
        Self {
            run_id,
            name: spdg.name,
            num_nodes: spdg.graph.node_count() as u32,
            statistics: spdg.statistics.clone(),
            max_inlining_depth: spdg.graph.node_weights().map(|w| w.at.len()).max().unwrap() as u16,
            avg_inlining_depth: inlining_sum as f32 / spdg.graph.node_count() as f32,
            num_edges: spdg.graph.edge_count() as u32,
        }
    }
}

#[derive(Clone, Copy)]
pub struct CmdStat {
    peak_cpu: f32,
    avg_cpu: f32,
    peak_mem: u64,
    avg_mem: u64,
    elapsed: TimeMeasurement,
}

impl CmdStat {
    pub fn for_self<R>(config: &Config, f: impl FnOnce() -> R) -> (R, Self) {
        let sync = sync::OnceLock::new();
        thread::scope(|scope| {
            let handle =
                scope.spawn(|| Self::collect(config, std::process::id(), || sync.get().is_some()));

            let result = f();
            sync.set(()).unwrap();

            let stats = handle.join().unwrap();

            (result, stats)
        })
    }

    pub fn for_process(config: &Config, process: &mut Child) -> std::io::Result<Self> {
        let pid = process.id();
        let stat = Self::collect(config, pid, || process.try_wait().unwrap().is_some());

        Ok(stat)
    }

    fn collect(config: &Config, pid: u32, mut poll: impl FnMut() -> bool) -> Self {
        let mut sys_stat = sysinfo::System::new();
        let pid = sysinfo::Pid::from_u32(pid);
        let mut sum_mem = 1;
        let mut num_samples = 0;
        let mut sum_cpu = 0.0_f32;
        let mut peak_cpu = 0.0_f32;
        let mut peak_mem = 0;
        let started = Instant::now();

        while !poll() {
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

        CmdStat {
            peak_cpu,
            peak_mem,
            avg_cpu: sum_cpu / num_samples.max(1) as f32,
            avg_mem: sum_mem / num_samples.max(1),
            elapsed: started.elapsed().into(),
        }
    }
}
