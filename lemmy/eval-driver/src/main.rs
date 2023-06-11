extern crate clap;
extern crate indicatif;
use clap::Parser;

use indicatif::ProgressBar;

use std::fmt::{Display, Write};
use std::str::FromStr;
use std::time::{Duration, SystemTime};

const CONFIGURATIONS: &'static [Property] = &[
    Property::Instance,
    Property::Community,
];

// Controllers are broken into batches.
// Batches are arbitrary, except all of a subfolder's controllers are kept together.
// The "correct" feature flag is for a controller that the Lemmy developers found and fixed themselves.
// e.g. user-login is the version before the bug fix, and user-login correct is the version after.

const BUG_1_CTRL_BATCH_1: &'static [&'static str] = &[
    "bug-1-code comment-like",
    "bug-1-code comment-mark-as-read",
    "bug-1-code comment-save",
    "bug-1-code comment-report-create",
    "bug-1-code comment-report-list",
    "bug-1-code comment-report-resolve",
    "bug-1-code community-add-mod",
    "bug-1-code community-ban",
    "bug-1-code community-block",
    "bug-1-code community-follow",
    "bug-1-code community-hide",
    "bug-1-code community-transfer",
    "bug-1-code notification-list-mentions",
    "bug-1-code notification-list-replies",
    "bug-1-code notification-mark-all-read",
    "bug-1-code notification-mark-mention-read",
    "bug-1-code notification-unread-count",
    "bug-1-code user-add-admin",
    "bug-1-code user-ban-person",
    "bug-1-code user-block",
    "bug-1-code user-change-password",
    "bug-1-code user-list-banned",
    "bug-1-code user-login",
    "bug-1-code user-report-count",
    "bug-1-code user-save-settings",
];

const BUG_1_CTRL_BATCH_2 : &'static [&'static str] = &[
    "bug-1-code post-like",
    "bug-1-code post-lock",
    "bug-1-code post-mark-read",
    "bug-1-code post-save",
    "bug-1-code post-sticky",
    "bug-1-code post-report-create",
    "bug-1-code post-report-list",
    "bug-1-code post-report-resolve",
    "bug-1-code private-message-mark-read",
    "bug-1-code purge-comment",
    "bug-1-code purge-community",
    "bug-1-code purge-person",
    "bug-1-code purge-post",
    "bug-1-code registration-approve",
    "bug-1-code registration-list",
    "bug-1-code registration-unread-counts",
    "bug-1-code site-leave-admin",
    "bug-1-code site-mod-log",
    "bug-1-code site-resolve-object",
    "bug-1-code site-search",
    "bug-1-code comment-create",
    "bug-1-code comment-create correct",
    "bug-1-code comment-delete",
    "bug-1-code comment-list",
    "bug-1-code comment-read",
    "bug-1-code comment-remove",
    "bug-1-code comment-update",
    "bug-1-code comment-update correct",
];

const BUG_1_CTRL_BATCH_3 : &'static [&'static str] = &[
    "bug-1-code community-create",
    "bug-1-code community-delete",
    "bug-1-code community-list",
    "bug-1-code community-read",
    "bug-1-code community-remove",
    "bug-1-code community-update",
    "bug-1-code post-create",
    "bug-1-code post-create correct",
    "bug-1-code post-delete",
    "bug-1-code post-delete correct",
    "bug-1-code post-list",
    "bug-1-code post-read",
    "bug-1-code post-remove",
    "bug-1-code post-update",
    "bug-1-code post-update correct",
    "bug-1-code private-message-create",
    "bug-1-code private-message-delete",
    "bug-1-code private-message-read",
    "bug-1-code private-message-update",
    "bug-1-code site-create",
    "bug-1-code site-read",
    "bug-1-code site-update",
    "bug-1-code user-delete",
    "bug-1-code user-read"
];

const BUG_1_FIX_CTRL_BATCH_1: &'static [&'static str] = &[
    "bug-1-code bug-1-fix comment-like",
    "bug-1-code bug-1-fix comment-mark-as-read",
    "bug-1-code bug-1-fix comment-save",
    "bug-1-code bug-1-fix comment-report-create",
    "bug-1-code bug-1-fix comment-report-list",
    "bug-1-code bug-1-fix comment-report-resolve",
    "bug-1-code bug-1-fix community-add-mod",
    "bug-1-code bug-1-fix community-ban",
    "bug-1-code bug-1-fix community-block",
    "bug-1-code bug-1-fix community-follow",
    "bug-1-code bug-1-fix community-hide",
    "bug-1-code bug-1-fix community-transfer",
    "bug-1-code bug-1-fix notification-list-mentions",
    "bug-1-code bug-1-fix notification-list-replies",
    "bug-1-code bug-1-fix notification-mark-all-read",
    "bug-1-code bug-1-fix notification-mark-mention-read",
    "bug-1-code bug-1-fix notification-unread-count",
    "bug-1-code bug-1-fix user-add-admin",
    "bug-1-code bug-1-fix user-ban-person",
    "bug-1-code bug-1-fix user-block",
    "bug-1-code bug-1-fix user-change-password",
    "bug-1-code bug-1-fix user-list-banned",
    "bug-1-code bug-1-fix user-login",
    "bug-1-code bug-1-fix user-report-count",
    "bug-1-code bug-1-fix user-save-settings",
];

const BUG_1_FIX_CTRL_BATCH_2 : &'static [&'static str] = &[
    "bug-1-code bug-1-fix post-like",
    "bug-1-code bug-1-fix post-lock",
    "bug-1-code bug-1-fix post-mark-read",
    "bug-1-code bug-1-fix post-save",
    "bug-1-code bug-1-fix post-sticky",
    "bug-1-code bug-1-fix post-report-create",
    "bug-1-code bug-1-fix post-report-list",
    "bug-1-code bug-1-fix post-report-resolve",
    "bug-1-code bug-1-fix private-message-mark-read",
    "bug-1-code bug-1-fix purge-comment",
    "bug-1-code bug-1-fix purge-community",
    "bug-1-code bug-1-fix purge-person",
    "bug-1-code bug-1-fix purge-post",
    "bug-1-code bug-1-fix registration-approve",
    "bug-1-code bug-1-fix registration-list",
    "bug-1-code bug-1-fix registration-unread-counts",
    "bug-1-code bug-1-fix site-leave-admin",
    "bug-1-code bug-1-fix site-mod-log",
    "bug-1-code bug-1-fix site-resolve-object",
    "bug-1-code bug-1-fix site-search",
    "bug-1-code bug-1-fix comment-create",
    "bug-1-code bug-1-fix comment-create correct",
    "bug-1-code bug-1-fix comment-delete",
    "bug-1-code bug-1-fix comment-list",
    "bug-1-code bug-1-fix comment-read",
    "bug-1-code bug-1-fix comment-remove",
    "bug-1-code bug-1-fix comment-update",
    "bug-1-code bug-1-fix comment-update correct",
];

const BUG_1_FIX_CTRL_BATCH_3 : &'static [&'static str] = &[
    "bug-1-code bug-1-fix community-create",
    "bug-1-code bug-1-fix community-delete",
    "bug-1-code bug-1-fix community-list",
    "bug-1-code bug-1-fix community-read",
    "bug-1-code bug-1-fix community-remove",
    "bug-1-code bug-1-fix community-update",
    "bug-1-code bug-1-fix post-create",
    "bug-1-code bug-1-fix post-create correct",
    "bug-1-code bug-1-fix post-delete",
    "bug-1-code bug-1-fix post-delete correct",
    "bug-1-code bug-1-fix post-list",
    "bug-1-code bug-1-fix post-read",
    "bug-1-code bug-1-fix post-remove",
    "bug-1-code bug-1-fix post-update",
    "bug-1-code bug-1-fix post-update correct",
    "bug-1-code bug-1-fix private-message-create",
    "bug-1-code bug-1-fix private-message-delete",
    "bug-1-code bug-1-fix private-message-read",
    "bug-1-code bug-1-fix private-message-update",
    "bug-1-code bug-1-fix site-create",
    "bug-1-code bug-1-fix site-read",
    "bug-1-code bug-1-fix site-update",
    "bug-1-code bug-1-fix user-delete",
    "bug-1-code bug-1-fix user-read"
];

const POST_BUG_1_CTRL_BATCH_1: &'static [&'static str] = &[
    "post-bug-1 comment-like",
    "post-bug-1 comment-mark-as-read",
    "post-bug-1 comment-save",
    "post-bug-1 comment-report-create",
    "post-bug-1 comment-report-list",
    "post-bug-1 comment-report-resolve",
    "post-bug-1 community-add-mod",
    "post-bug-1 community-ban",
    "post-bug-1 community-block",
    "post-bug-1 community-follow",
    "post-bug-1 community-hide",
    "post-bug-1 community-transfer",
    "post-bug-1 notification-list-mentions",
    "post-bug-1 notification-list-replies",
    "post-bug-1 notification-mark-all-read",
    "post-bug-1 notification-mark-mention-read",
    "post-bug-1 notification-unread-count",
    "post-bug-1 user-add-admin",
    "post-bug-1 user-ban-person",
    "post-bug-1 user-block",
    "post-bug-1 user-change-password",
    "post-bug-1 user-list-banned",
    "post-bug-1 user-login correct",
    "post-bug-1 user-report-count",
    "post-bug-1 user-save-settings",
];

const POST_BUG_1_CTRL_BATCH_2 : &'static [&'static str] = &[
    "post-bug-1 post-like",
    "post-bug-1 post-lock",
    "post-bug-1 post-mark-read",
    "post-bug-1 post-save",
    "post-bug-1 post-sticky",
    "post-bug-1 post-report-create",
    "post-bug-1 post-report-list",
    "post-bug-1 post-report-resolve",
    "post-bug-1 private-message-mark-read",
    "post-bug-1 purge-comment",
    "post-bug-1 purge-community",
    "post-bug-1 purge-person",
    "post-bug-1 purge-post",
    "post-bug-1 registration-approve",
    "post-bug-1 registration-list",
    "post-bug-1 registration-unread-counts",
    "post-bug-1 site-leave-admin",
    "post-bug-1 site-mod-log",
    "post-bug-1 site-resolve-object",
    "post-bug-1 site-search",
    "post-bug-1 comment-create",
    "post-bug-1 comment-create correct",
    "post-bug-1 comment-delete",
    "post-bug-1 comment-list",
    "post-bug-1 comment-read",
    "post-bug-1 comment-remove",
    "post-bug-1 comment-update",
    "post-bug-1 comment-update correct",
];

const POST_BUG_1_CTRL_BATCH_3 : &'static [&'static str] = &[
    "post-bug-1 community-create",
    "post-bug-1 community-delete",
    "post-bug-1 community-list",
    "post-bug-1 community-read",
    "post-bug-1 community-remove",
    "post-bug-1 community-update",
    "post-bug-1 post-create",
    "post-bug-1 post-create correct",
    "post-bug-1 post-delete",
    "post-bug-1 post-delete correct",
    "post-bug-1 post-list",
    "post-bug-1 post-read",
    "post-bug-1 post-remove",
    "post-bug-1 post-update",
    "post-bug-1 post-update correct",
    "post-bug-1 private-message-create",
    "post-bug-1 private-message-delete",
    "post-bug-1 private-message-read",
    "post-bug-1 private-message-update",
    "post-bug-1 site-create",
    "post-bug-1 site-read",
    "post-bug-1 site-update",
    "post-bug-1 user-delete",
    "post-bug-1 user-read"
];

// no bug 1 batch because bug 1 involves all of the controllers

const BUG_2_BATCH : &'static [&'static str] = &[
    "post-bug-1 user-login",
    "post-bug-1 user-login correct",
];

// these are the buggy controllers that the Lemmy developers found and fixed themselves
const BUG_3_BUGGY_BATCH : &'static [&'static str] = &[
    "post-bug-1 comment-create",
    "post-bug-1 comment-create correct",
    "post-bug-1 comment-update",
    "post-bug-1 comment-update correct",
    "post-bug-1 post-create",
    "post-bug-1 post-create correct",
    "post-bug-1 post-delete",
    "post-bug-1 post-delete correct",
    "post-bug-1 post-update",
    "post-bug-1 post-update correct",
];

// these are the buggy controllers that Paralegal found
const BUG_3_BATCH : &'static [&'static str] = &[
    "post-bug-1 comment-like",
    "post-bug-1 comment-mark-as-read",
    "post-bug-1 comment-save",
    "post-bug-1 comment-report-create",
    "post-bug-1 community-block",
    "post-bug-1 community-follow",
    "post-bug-1 post-mark-read",
    "post-bug-1 post-save",
    "post-bug-1 post-report-create",
    "post-bug-1 post-report-resolve",
    "post-bug-1 comment-delete",
    "post-bug-1 comment-remove",
    "post-bug-1 community-delete",
    "post-bug-1 community-remove",
    "post-bug-1 community-update",
    "post-bug-1 post-remove"
];

const BUG_4_BATCH : &'static [&'static str] = &[
    "post-bug-1 community-add-mod",
    "post-bug-1 community-ban",
    "post-bug-1 community-hide",
    "post-bug-1 community-transfer"
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
    
    /// Whether to run all the controllers at once; default is to only run ones relevant to each bug
    #[clap(long)]
    all: bool,

    /// Print the shell commands we are running
    #[clap(long)]
    verbose_commands: bool,

    #[clap(long)]
    /// Controllers to run in api
    ctrlers: Vec<String>,
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

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
enum GetUserVersion {
    PreBug1Fix,
    PostBug1Fix,
    Bug2Onward,
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
    verbose: bool,
    verbose_commands: bool,
    progress: &ProgressBar,
) -> Vec<RunResult> {
    use std::process::*;

    ctrlers
        .iter()
        .map(|ctrler| {
            let mut dfpp_cmd = Command::new("cargo");
            dfpp_cmd.current_dir("../").arg("dfpp").stdin(Stdio::null());

			dfpp_cmd.args(&["--target", "lemmy_api"]);
            dfpp_cmd.args(&["--model-version", "v2"]);
            dfpp_cmd.args(&["--inline-elision"]);
            dfpp_cmd.args(&["--abort-after-analysis"]);

            let external_ann_file_name = format!("external-annotations.toml");
            let mut external_ann_file: std::path::PathBuf = "../".into();
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
                .current_dir("../")
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
    batch : &Vec<String>,
    result: (&Property, Vec<RunResult>),
    desc: &'static str,
) -> std::io::Result<()> {
    let leftmost_column_width = 40;
    let rest_column_width = 12;

    write!(w, "{}", desc)?;
    writeln!(w, "")?;

    // headers
    write!(w, " {:leftmost_column_width$} ", typ.to_string())?;
    write!(w, "| {:rest_column_width$} ", "pass?")?;
    write!(w, "| {:rest_column_width$} ", "atime")?;
    write!(w, "| {:rest_column_width$} ", "vtime")?;
    writeln!(w, "")?;
    
    // dividing line
    write!(w, "-{:-<leftmost_column_width$}-", "")?;
    for _ in 0..3 {
        write!(w, "+-{:-<rest_column_width$}-", "")?
    }
    writeln!(w, "")?;

    let (_, versions) = result;

    let mut i : usize = 0;

    // each row : controller, result, analyze time, verification time
    for result in versions.clone().into_iter() {
        write!(w, " {:leftmost_column_width$} ", batch[i])?;
        write!(w, "| {:^rest_column_width$} ", result.error)?;
        write!(w, "| {:^rest_column_width$} ", format!("{:?}", result.analyze_time))?;
        write!(w, "| {:^rest_column_width$} ", format!("{:?}", result.verify_time))?;

        // dividing line
        writeln!(w, "")?;
        write!(w, "-{:-<leftmost_column_width$}-", "")?;
        for _ in 0..3 {
            write!(w, "+-{:-<rest_column_width$}-", "")?;
        }
        writeln!(w, "")?;
        i += 1;
    }
    writeln!(w, "")?;
    Ok(())
}

// helper function; runs one batch of controllers
fn run_batch(args : &Args, batch : &Vec<String>, desc: &'static str) {
    let num_versions = batch.len();

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
                        batch.as_slice(),
                        args.verbose,
                        args.verbose_commands(),
                        &progress,
                    ),
                );
        progress.suspend(|| {
            print_results_for_property(&mut w, num_versions, typ, batch, results, desc)
                .unwrap()
        })
    }
    progress.finish_and_clear();
}

// runs all controllers
fn run_all(args: &Args, version: GetUserVersion) {
    if version == GetUserVersion::PreBug1Fix {
        run_batch(args, &(BUG_1_CTRL_BATCH_1.iter().cloned().map(str::to_string).collect()), "Bug 1: Batch 1 Results");
        run_batch(args, &(BUG_1_CTRL_BATCH_2.iter().cloned().map(str::to_string).collect()), "Bug 1: Batch 2 Results");
        run_batch(args, &(BUG_1_CTRL_BATCH_3.iter().cloned().map(str::to_string).collect()), "Bug 1: Batch 3 Results");
    } else if version == GetUserVersion::PostBug1Fix {
        run_batch(args, &(BUG_1_FIX_CTRL_BATCH_1.iter().cloned().map(str::to_string).collect()), "Bug 1 Fix: Batch 1 Results");
        run_batch(args, &(BUG_1_FIX_CTRL_BATCH_2.iter().cloned().map(str::to_string).collect()), "Bug 1 Fix: Batch 2 Results");
        run_batch(args, &(BUG_1_FIX_CTRL_BATCH_3.iter().cloned().map(str::to_string).collect()), "Bug 1 Fix: Batch 3 Results");
    } else {
        run_batch(args, &(POST_BUG_1_CTRL_BATCH_1.iter().cloned().map(str::to_string).collect()), "Batch 1 Results:");
        run_batch(args, &(POST_BUG_1_CTRL_BATCH_2.iter().cloned().map(str::to_string).collect()), "Batch 2 Results:");
        run_batch(args, &(POST_BUG_1_CTRL_BATCH_3.iter().cloned().map(str::to_string).collect()), "Batch 3 Results:");
    }
}

// Runs controllers relevant for each bug (1-4)
// For Bug 1, this is all of the controllers twice: once before the bug fix, once after
// For Bug 2, this is login twice: once before the bug fix, once after
// For Bug 3, this is two batches: the controllers the Lemmy developers found, and the ones Paralegal found.
// For the controllers that the Lemmy developers found, each controller runs once before the bug fix, once after
// For Bug 4, this is once batch: the controllers Paralegal found
fn run_bugs(args: &Args) {
    run_all(args, GetUserVersion::PreBug1Fix);
    run_all(args, GetUserVersion::PostBug1Fix);
    run_batch(args, &(BUG_2_BATCH.iter().cloned().map(str::to_string).collect()), "Bug 2 Batch");
    run_batch(args, &(BUG_3_BUGGY_BATCH.iter().cloned().map(str::to_string).collect()), "Bug 3 Batch -- Lemmy developers found and fixed");
    run_batch(args, &(BUG_3_BATCH.iter().cloned().map(str::to_string).collect()), "Bug 3 Batch -- Paralegal found");
    run_batch(args, &(BUG_4_BATCH.iter().cloned().map(str::to_string).collect()), "Bug 4 Batch");
}

fn main() {
    // use std::io::Write;
    let args = Args::parse();
    
    if args.all {
        println!("INFO: Running all controllers -- note that this is the Lemmy version for bugs 2-4.");
        run_all(&args, GetUserVersion::Bug2Onward);
    } else if args.ctrlers.is_empty() {
        println!("INFO: No controllers specified; running relevant controllers for each bug");
        run_bugs(&args)
    } else {
        println!("INFO: Running specified controllers.");
        run_batch(&args, &args.ctrlers, "");
    }
}
