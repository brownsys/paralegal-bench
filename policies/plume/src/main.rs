use anyhow::Result;
use clap::Parser;

use plume::PlumeVersion;

#[derive(clap::Parser)]
struct Args {
    plume_dir: std::path::PathBuf,
    /// Which plume version to run.
    ///
    /// - `v0` is the original version that deletes no comments
    /// - `v1` deletes the comments
    /// - `v2` includes the requirement to delete media
    /// - `v3` also ensures the media is deleted
    #[clap(long, short = 'p', default_value_t = PlumeVersion::V0, value_enum)]
    plume_version: PlumeVersion,
    /// Additional arguments to pass to cargo, this is intended to be used to
    /// enable the features that toggle the bugs, like `delete-comments`.
    #[clap(last = true)]
    cargo_args: Vec<String>,
}

fn main() -> Result<()> {
    let args = Args::try_parse()?;

    let mut cmd = paralegal_policy::SPDGGenCommand::global();
    cmd.get_command().args([
        "--external-annotations",
        "external-annotations.toml",
        "--abort-after-analysis",
        "--target",
        "plume-models",
        "--",
        "--no-default-features",
        "--features",
        "postgres",
    ]);
    for (version_bound, feature) in [
        (PlumeVersion::V1, "delete-comments"),
        (PlumeVersion::V2, "require-delete-media"),
        (PlumeVersion::V3, "delete-media"),
    ] {
        if args.plume_version >= version_bound {
            cmd.get_command()
                .args(["--features", &format!("plume-models/{feature}")]);
        }
    }
    cmd.get_command().args(args.cargo_args);
    let result = cmd.run(args.plume_dir)?.with_context(plume::check)?;
    println!(
        "Finished {}successfully with {}",
        if result.success { "" } else { "un" },
        result.stats
    );
    if !result.success {
        std::process::exit(1);
    }
    Ok(())
}
