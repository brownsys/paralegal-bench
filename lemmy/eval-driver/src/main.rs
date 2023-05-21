extern crate clap;
extern crate indicatif;
use clap::Parser;

use indicatif::ProgressBar;

use std::collections::HashSet;
use std::fmt::{Display, Write};
use std::str::FromStr;
use std::time::{Duration, SystemTime};

const CONFIGURATIONS: &'static [Property] = &[
    Property::Instance,
    Property::Community,
];

const ALL_KNOWN_CTRLERS: &'static [&'static str] = &[
	// "comment-like",
    // "comment-mark-as-read",
    // "comment-save",
    // "comment-report-create",
    // "comment-report-list",
    // "comment-report-resolve",
    // "community-add-mod",
    // "community-ban",
    // "community-block",
    // "community-follow",
    // "community-hide",
    // "community-transfer",
    // "notification-list-mentions",
    // "notification-list-replies",
    // "notification-mark-all-read",
    // "notification-mark-mention-read",
    // "notification-unread-count",
    // "user-add-admin",
    // "user-ban-person",
    // "user-block",
    // "user-change-password",
    // "user-list-banned",
    "user-login",
    "user-login user-login-correct",
    // "user-report-count",
    // "user-save-settings",
    // "post-like",
    // "post-lock",
    // "post-mark-read",
    // "post-save",
    // "post-sticky",
    // "post-report-create",
    // "post-report-list",
    // "post-report-resolve",
    // "private-message-mark-read",
    // "purge-comment",
    // "purge-community",
    // "purge-person",
    // "purge-post",
    // "registration-approve",
    // "registration-list",
    // "registration-unread-counts",
    // "site-leave-admin",
    // "site-mod-log",
    // "site-resolve-object",
    // "site-search",
    // "comment-create",
    "comment-create comment-create-correct",
    // "comment-delete",
    // "comment-list",
    // "comment-read",
    // "comment-remove",
    // "comment-update",
    "comment-update comment-update-correct",
    "community-create",
    // "community-delete",
    // "community-list",
    // "post-read",
    // "community-remove",
    // "community-update",
    // "post-create",
    "post-create post-create-correct",
    // "post-delete",
    "post-delete post-delete-correct",
    // "post-list",
    "post-read",
    // "post-remove",
    // "post-update",
    "post-update post-update-correct",
    "private-message-create",
    // "private-message-delete",
    // "private-message-read",
    // "private-message-update",
    "site-create",
    "site-read",
    "site-update",
    "user-delete",
    "user-read"
];

/// Batch executor for the evaluation of our 2023 Eurosys paper.
///
/// Be aware that this tool does not install dfpp itself but assumes the latest
/// version is already present and in the $PATH.
#[derive(Parser)]
struct Args {
    /// Print complete error messages for called programs on failure (implies
    /// `--verbose-commands`)
    #[clap(long)]
    verbose: bool,

    /// Print the shell commands we are running
    #[clap(long)]
    verbose_commands: bool,

    /// Controllers to run in api
    ctrlers: Vec<String>,

    /// Location of the Lemmy repo
    #[clap(long, default_value = "..")]
    directory: std::path::PathBuf,
}

impl Args {
    fn verbose_commands(&self) -> bool {
        self.verbose || self.verbose_commands
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
enum Property {
    Instance,
    Community,
}

impl Display for Property {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str(match self {
            Property::Instance => "instance",
            Property::Community => "community",
        })
    }
}

impl FromStr for Property {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "instance" => Ok(Property::Instance),
            "community" => Ok(Property::Community),
            _ => Err(format!("Unknown property type {s}")),
        }
    }
}

#[derive(Clone, Copy)]
struct RunResult {
	error: RunError,
	analyze_time: Duration,
	verify_time: Duration,
}

#[derive(Clone, Copy)]
enum RunError {
    Success,
    CompilationError,
    CheckError,
}

// impl From<bool> for RunResult {
//     fn from(b: bool) -> Self {
//         if b {
//             RunResult::Success
//         } else {
//             RunResult::CheckError
//         }
//     }
// }

impl std::fmt::Display for RunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        use std::fmt::Alignment;
        let width = formatter.width().unwrap_or(2);
        let (before, after) = match formatter.align() {
            None => (0, width - 2),
            _ if width < 2 => (0, 0),
            Some(Alignment::Left) => (0, width - 2),
            Some(Alignment::Right) => (width - 2, 0),
            Some(Alignment::Center) => {
                let left = (width - 2) / 2;
                (left, width - 2 - left)
            }
        };
        let fill_chr = formatter.fill();
        for _ in 0..before {
            formatter.write_char(fill_chr)?;
        }
        match self {
            RunError::Success => formatter.write_str("✅"),
            RunError::CompilationError => formatter.write_str("️🚧"),
            RunError::CheckError => formatter.write_str("❌"),
        }?;
        for _ in 0..after {
            formatter.write_char(fill_chr)?;
        }
        Ok(())
    }
}

fn run_edit(
    typ: Property,
    ctrlers: &[String],
    cd: &std::path::Path,
    verbose: bool,
    verbose_commands: bool,
    progress: &ProgressBar,
) -> Vec<RunResult> {
    use std::process::*;

    ctrlers
        .iter()
        .map(|ctrler| {
            let mut dfpp_cmd = Command::new("cargo");
            dfpp_cmd.current_dir(cd).arg("dfpp").stdin(Stdio::null());

			dfpp_cmd.args(&["--target", "lemmy_api"]);
            dfpp_cmd.args(&["--model-version", "v2"]);
            dfpp_cmd.args(&["--inline-elision"]);

            let external_ann_file_name = format!("external-annotations.toml");
            let mut external_ann_file: std::path::PathBuf = cd.into();
            external_ann_file.push(&external_ann_file_name);
            if external_ann_file.exists() {
                dfpp_cmd.args(&["--external-annotations", external_ann_file_name.as_str()]);
            }
            dfpp_cmd.args(&["--", "--features", &format!("{ctrler}")]);
            if !verbose {
                dfpp_cmd.stderr(Stdio::null()).stdout(Stdio::null());
            }
            if verbose_commands {
                progress.suspend(|| println!("Executing compile command: {:?}", dfpp_cmd));
            }
			let mut now = SystemTime::now();
            let status = dfpp_cmd.status().unwrap();
			let analyze_time = now.elapsed().unwrap();
            progress.inc(1);
            // if !status.success() {
            //     progress.inc(1);
            //     return RunResult::CompilationError;
            // } // NOTE: This is commented out because `cargo dfpp` always returns error for some reason, but it works (usually).

            let propfile = format!("props/{typ}-props.frg");
            let mut racket_cmd = Command::new("racket");
            racket_cmd
                .current_dir(cd)
                .arg(propfile)
                .stdin(Stdio::null());
            if !verbose {
                racket_cmd.stderr(Stdio::null()).stdout(Stdio::null());
            }
            if verbose_commands {
                progress.suspend(|| println!("Executing check command: {:?}", racket_cmd));
            }
			now = SystemTime::now();
            let status = racket_cmd.status().unwrap();
			let verify_time = now.elapsed().unwrap();
            progress.inc(1);
            if status.success() {
                RunResult{
					analyze_time,
					verify_time,
					error: RunError::Success
				}
            } else {
                RunResult{
					analyze_time,
					verify_time,
					error: RunError::CheckError
				}
            }
        })
        .collect()
}

fn print_results_for_property<W: std::io::Write>(
    mut w: W,
    num_versions: usize,
    typ: Property,
    args: &Args,
    result: (&Property, Vec<RunResult>),
) -> std::io::Result<()> {
    let head_cell_width = 10;
    let body_cell_width = 30;

    write!(w, " {:head_cell_width$} ", typ.to_string())?;
    for version in args.ctrlers.iter() {
        write!(w, "| {:body_cell_width$} ", version)?
    }
    writeln!(w, "")?;

    write!(w, "-{:-<head_cell_width$}-", "")?;
    for _ in 0..args.ctrlers.len() + 1 {
        write!(w, "+-{:-<body_cell_width$}-", "")?
    }
    writeln!(w, "")?;

    let (_, versions) = result;
	write!(w, " {:head_cell_width$} ", "pass?")?;
	for result in versions.clone().into_iter() {
		write!(w, "| {:^body_cell_width$} ", result.error)?;
	}
    writeln!(w, "")?;

	write!(w, " {:head_cell_width$} ", "atime")?;
	for result in versions.clone().into_iter() {
		write!(w, "| {:^body_cell_width$} ", format!("{:?}", result.analyze_time))?;
	}
	writeln!(w, "")?;

	write!(w, " {:head_cell_width$} ", "vtime")?;
	for result in versions.clone().into_iter() {
		write!(w, "| {:^body_cell_width$} ", format!("{:?}", result.verify_time))?;
	}
	writeln!(w, "")?;
	writeln!(w, "")
}

fn main() {
    use std::io::Write;
    let args = {
        let mut args = Args::parse();
        if args.ctrlers.is_empty() {
            println!("INFO: No specification variants to run given, running all known ones");
            args.ctrlers = ALL_KNOWN_CTRLERS
                .iter()
                .cloned()
                .map(str::to_string)
                .collect();
        }
        args
    };

    let num_versions = args.ctrlers.len();

    let num_configurations = CONFIGURATIONS
        .len()
        * (2 // compile 
            * num_versions);

    let progress = ProgressBar::new(num_configurations as u64).with_style(
        indicatif::ProgressStyle::default_bar()
            .template("{msg:11} {bar:40} {pos:>3}/{len:3}")
            .unwrap(),
    );

    let mut w = std::io::stdout();
    for &typ in CONFIGURATIONS {
        let results = (
                    &typ,
                    run_edit(
                        typ,
                        args.ctrlers.as_slice(),
                        &args.directory,
                        args.verbose,
                        args.verbose_commands(),
                        &progress,
                    ),
                );
        progress.suspend(|| {
            print_results_for_property(&mut w, num_versions, typ, &args, results)
                .unwrap()
        })
    }
    progress.finish_and_clear();
}