use input::Config;
use output::{CmdStat, ControllerStat, RunStat};
use paralegal_policy::GraphLocation;
use run::Output;
use std::sync::Arc;
use std::time::Instant;

pub mod conversion;
pub mod input;
pub mod output;
pub mod run;

fn main() {
    let mut output = Output::init().unwrap();
    let config_file = std::fs::read_to_string("bench-config.toml").unwrap();
    let config: Config = toml::from_str(&config_file).unwrap();

    for (id, mut exp) in config.experiments().enumerate() {
        if let Some(prepare) = exp.prepare.as_ref() {
            (prepare)()
        }
        let compile_command = &mut exp.compile_cmd;
        let compile_dir = &exp.app_config.source_dir;
        let mut process = compile_command.get_command().spawn().unwrap();
        let cmd_stat = CmdStat::for_process(&config, &mut process).unwrap();
        let mut run_stats = RunStat::from_experiment(id as u32, &exp, cmd_stat);
        if process.try_wait().unwrap().unwrap().success() {
            let policy = exp.policy;
            let ((ctx, success, traversal_time), cmd_stat) = CmdStat::for_self(&config, || {
                let ctx = Arc::new(
                    GraphLocation::std(compile_dir)
                        .build_context(paralegal_policy::Config::default())
                        .unwrap(),
                );
                let policy_start = Instant::now();
                (policy)(ctx.clone()).unwrap();
                let success = ctx.emit_diagnostics(std::io::stdout()).unwrap();
                (ctx, success, policy_start.elapsed())
            });
            run_stats.add_policy_stat(cmd_stat, ctx.as_ref(), success, traversal_time);
            for ctrl in ctx.desc().controllers.values() {
                output
                    .controller_stat_out
                    .serialize(ControllerStat::from_spdg(id as u32, ctrl))
                    .unwrap()
            }
        } else {
            println!(
                "WARNING: Run id {} dir not successfully pass PDG construction",
                id
            );
        }
        output.run_stat_out.serialize(run_stats).unwrap();
        output.flush().unwrap();
    }
}
