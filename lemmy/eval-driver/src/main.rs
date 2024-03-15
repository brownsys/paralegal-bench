extern crate clap;
extern crate indicatif;
use clap::{Parser, ValueEnum};

use indicatif::ProgressBar;

use std::fmt::{Display, Write};
use std::str::FromStr;
use std::time::{Duration, SystemTime};

const CONFIGURATIONS: &'static [Property] = &[Property::Instance, Property::Community];

// Controllers are broken into batches.
// Batches are arbitrary, except all of a subfolder's controllers are kept together.
// The "correct" feature flag is for a controller that the Lemmy developers found and fixed themselves.
// e.g. user-login is the version before the bug fix, and user-login correct is the version after.

// used to all have "post-bug-1" prepended
/// Batches to run all of the controllers for Lemmy version bugs 2-4
const POST_BUG_1_BATCH_1: &'static [&'static str] = &[
    "comment-like",
    "comment-mark-as-read",
    "comment-save",
    "comment-report-create",
    "comment-report-list",
    "comment-report-resolve",
    "community-add-mod",
    "community-ban",
    "community-block",
    "community-follow",
    "community-hide",
    "community-transfer",
    "notification-list-mentions",
    "notification-list-replies",
    "notification-mark-all-read",
    "notification-mark-mention-read",
    "notification-unread-count",
    "user-add-admin",
    "user-ban-person",
    "user-block",
    "user-change-password",
    "user-list-banned",
    "user-login correct",
    "user-report-count",
    "user-save-settings",
];

// used to all have "post-bug-1" prepended
const POST_BUG_1_BATCH_2: &'static [&'static str] = &[
    "post-like",
    "post-lock",
    "post-mark-read",
    "post-save",
    "post-sticky",
    "post-report-create",
    "post-report-list",
    "post-report-resolve",
    "private-message-mark-read",
    "purge-comment",
    "purge-community",
    "purge-person",
    "purge-post",
    "registration-approve",
    "registration-list",
    "registration-unread-counts",
    "site-leave-admin",
    "site-mod-log",
    "site-resolve-object",
    "site-search",
    // "comment-create", times out
    // "comment-create correct", times out
    "comment-delete",
    "comment-list",
    "comment-read",
    "comment-remove",
    "comment-update",
    "comment-update correct",
];

// used to all have "post-bug-1" prepended
const POST_BUG_1_BATCH_3: &'static [&'static str] = &[
    "community-create",
    "community-delete",
    "community-list",
    "community-read",
    "community-remove",
    "community-update",
    "post-create",
    "post-create correct",
    "post-delete",
    "post-delete correct",
    "post-list",
    "post-read",
    "post-remove",
    "post-update",
    "post-update correct",
    "private-message-create",
    "private-message-delete",
    "private-message-read",
    "private-message-update",
    "site-create",
    // "site-read", times out
    "site-update",
    "user-delete",
    "user-read",
];

// all used to have "bug-1-code" prepended
/// Batches for each bug
const BUG_1_BATCH_1: &'static [&'static str] = &[
    "comment-like",
    "comment-mark-as-read",
    "comment-save",
    "comment-report-create",
    "comment-report-list",
    "comment-report-resolve",
    "community-add-mod",
    "community-ban",
    "community-block",
    "community-follow",
    "community-hide",
    "community-transfer",
    "notification-list-mentions",
    "notification-list-replies",
    "notification-mark-all-read",
    "notification-mark-mention-read",
    "notification-unread-count",
    "user-add-admin",
    "user-ban-person",
    "user-block",
    "user-change-password",
    "user-list-banned",
    "user-login",
    "user-report-count",
    "user-save-settings",
];

// used to all have "post-bug-1" prepended
const BUG_1_BATCH_2: &'static [&'static str] = &[
    "post-like",
    "post-lock",
    "post-mark-read",
    "post-save",
    "post-sticky",
    "post-report-create",
    "post-report-list",
    "post-report-resolve",
    "private-message-mark-read",
    "purge-comment",
    "purge-community",
    "purge-person",
    "purge-post",
    "registration-approve",
    "registration-list",
    "registration-unread-counts",
    "site-leave-admin",
    "site-mod-log",
    "site-resolve-object",
    "site-search",
    // "comment-create", times out
    "comment-delete",
    "comment-list",
    "comment-read",
    "comment-remove",
    "comment-update",
];

// used to all have "post-bug-1" prepended
const BUG_1_BATCH_3: &'static [&'static str] = &[
    "community-create",
    "community-delete",
    "community-list",
    "community-read",
    "community-remove",
    "community-update",
    "post-create",
    "post-delete",
    "post-list",
    "post-read",
    "post-remove",
    "post-update",
    "private-message-create",
    "private-message-delete",
    "private-message-read",
    "private-message-update",
    "site-create",
    // "site-read", times out
    "site-update",
    "user-delete",
    "user-read",
];

// used to all have "bug-1-code bug-1-fix" prepended
const BUG_1_FIX_BATCH_1: &'static [&'static str] = &[
    "comment-like",
    "comment-mark-as-read",
    "comment-save",
    "comment-report-create",
    "comment-report-list",
    "comment-report-resolve",
    "community-add-mod",
    "community-ban",
    "community-block",
    "community-follow",
    "community-hide",
    "community-transfer",
    "notification-list-mentions",
    "notification-list-replies",
    "notification-mark-all-read",
    "notification-mark-mention-read",
    "notification-unread-count",
    "user-add-admin",
    "user-ban-person",
    "user-block",
    "user-change-password",
    "user-list-banned",
    "user-login",
    "user-report-count",
    "user-save-settings",
];

// used to all have "bug-1-code bug-1-fix" prepended
const BUG_1_FIX_BATCH_2: &'static [&'static str] = &[
    "post-like",
    "post-lock",
    "post-mark-read",
    "post-save",
    "post-sticky",
    "post-report-create",
    "post-report-list",
    "post-report-resolve",
    "private-message-mark-read",
    "purge-comment",
    "purge-community",
    "purge-person",
    "purge-post",
    "registration-approve",
    "registration-list",
    "registration-unread-counts",
    "site-leave-admin",
    "site-mod-log",
    "site-resolve-object",
    "site-search",
    // "comment-create", times out
    "comment-delete",
    "comment-list",
    "comment-read",
    "comment-remove",
    "comment-update",
];

// used to all have "bug-1-code bug-1-fix" prepended
const BUG_1_FIX_BATCH_3: &'static [&'static str] = &[
    "community-create",
    "community-delete",
    "community-list",
    "community-read",
    "community-remove",
    "community-update",
    "post-create",
    "post-delete",
    "post-list",
    "post-read",
    "post-remove",
    "post-update",
    "private-message-create",
    "private-message-delete",
    "private-message-read",
    "private-message-update",
    "site-create",
    // "site-read", times out
    "site-update",
    "user-delete",
    "user-read",
];

// fn run_batch(
//     args: &Args,
//     batch: &[impl AsRef<str>],
//     props: &'static [Property],
//     desc: &'static str,
//     expect_failure: bool,

struct BatchConfig<'a> {
    baseline_feature: &'a str,
    expect_failure: bool,
    property: Property,
    comment: Option<&'a str>,
    baseline_controllers: &'a [&'a [&'a str]],
    change: Option<Change<'a>>,
}

struct Change<'a> {
    change_feature: &'a str,
    add_feature: bool,
    /// If this is `None`, all baseline controllers are affected.
    ///
    /// If this is `Some` it will contain a list of controllers that are changed with this feaure.
    /// That list of controllers should *not* be contained in the `baseline_controllers`
    /// field in [`BatchConfig`].
    affected_controllers: Option<&'a [&'a str]>,
}

const BUG_1_CONFIG: BatchConfig<'static> = BatchConfig {
    baseline_feature: "bug-1-code",
    expect_failure: true,
    property: Property::Instance,
    baseline_controllers: &[BUG_1_BATCH_1, BUG_1_BATCH_2, BUG_1_BATCH_3],
    comment: None,
    change: Some(Change {
        change_feature: "bug-1-fix",
        add_feature: true,
        affected_controllers: None,
    }),
};

const BUG_2_CONFIG: BatchConfig<'static> = BatchConfig {
    baseline_feature: "post-bug-1",
    expect_failure: false,
    property: Property::Instance,
    baseline_controllers: &[POST_BUG_1_BATCH_1, POST_BUG_1_BATCH_2, POST_BUG_1_BATCH_3],
    comment: None,
    change: Some(Change {
        change_feature: "correct",
        add_feature: false,
        affected_controllers: Some(&["user-login"]),
    }),
};

const BUG_3_CONFIG: BatchConfig<'static> = BatchConfig {
    baseline_feature: "post-bug-1",
    expect_failure: true,
    property: Property::Community,
    baseline_controllers: &[BUG_3_BATCH],
    comment: Some("Lemmy developers found and fixed"),
    change: Some(Change {
        change_feature: "correct",
        add_feature: true,
        affected_controllers: Some(BUG_3_FIXED_BATCH),
    }),
};

const BUG_4_CONFIG: BatchConfig<'static> = BatchConfig {
    baseline_feature: "post-bug-1",
    expect_failure: true,
    property: Property::Community,
    comment: Some("Paralegal found"),
    baseline_controllers: &[BUG_4_BATCH],
    change: None,
};

// Bug 1 all fail
// Bug fix all pass
// Bug 2 succeeds with "correct"
// Bug 3 one of them succeds with "correct"
// Bug 3 the other one always fails
// Bug 4 always fails

const BUG_2_BATCH: &'static [&'static str] =
    &["post-bug-1 user-login", "post-bug-1 user-login correct"];

// used to all have "post-bug-1" prepended
// these are the buggy controllers that the Lemmy developers found and fixed themselves
const BUG_3_FIXED_BATCH: &'static [&'static str] = &[
    // "comment-create", times out
    // "comment-create correct", times out
    "comment-update",
    "comment-update correct",
    "post-create",
    "post-create correct",
    "post-delete",
    "post-delete correct",
    "post-update",
    "post-update correct",
];

// used to all have "post-bug-1" prepended
// these are the buggy controllers that Paralegal found
const BUG_3_BATCH: &'static [&'static str] = &[
    "comment-like",
    "comment-mark-as-read",
    "comment-save",
    "comment-report-create",
    "community-block",
    "community-follow",
    "post-mark-read",
    "post-save",
    "post-report-create",
    "post-report-resolve",
    "comment-delete",
    "comment-remove",
    "community-delete",
    "community-remove",
    "community-update",
    "post-remove",
];

// used to all have "post-bug-1" prepended
const BUG_4_BATCH: &'static [&'static str] = &[
    "community-add-mod",
    "community-ban",
    "community-hide",
    "community-transfer",
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

    #[clap(long)]
    /// Specific controllers to run. Valid options are those in each BATCH list (see main.rs).
    ctrlers: Vec<String>,

    /// Bug(s) to verify. Defaults to all
    #[clap(long, value_enum)]
    bug: Vec<GetUserVersion>,

    lemmy_prop_dir: std::path::PathBuf,
}

impl Args {
    fn verbose_commands(&self) -> bool {
        self.verbose || self.verbose_commands
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Hash, strum::AsRefStr, strum::Display)]
#[strum(serialize_all = "kebab-case")]
enum Property {
    Instance,
    Community,
}

#[derive(Clone, Copy, Eq, PartialEq, Hash, strum::Display, clap::ValueEnum, Debug)]
#[clap(rename_all = "kebab-case")]
enum GetUserVersion {
    Bug1,
    Bug2,
    Bug3,
    Bug4,
}

impl GetUserVersion {
    fn to_config(self) -> &'static BatchConfig<'static> {
        match self {
            GetUserVersion::Bug1 => &BUG_1_CONFIG,
            GetUserVersion::Bug2 => &BUG_2_CONFIG,
            GetUserVersion::Bug3 => &BUG_3_CONFIG,
            GetUserVersion::Bug4 => &BUG_4_CONFIG,
        }
    }
}

#[derive(Clone, Copy)]
struct RunResult {
    error: RunError,
    analyze_time: Duration,
    verify_time: Duration,
    expected_outcome: bool,
}

impl RunResult {
    fn conforms_emoji(&self) -> &'static str {
        if self.error.conforms(self.expected_outcome) {
            "✅"
        } else {
            "❌"
        }
    }
}

#[derive(Clone, Copy)]
enum RunError {
    Success,
    CompilationError,
    CheckError,
}

impl RunError {
    fn conforms(self, expected: bool) -> bool {
        match (self, expected) {
            (RunError::Success, true) | (RunError::CheckError, false) => true,
            _ => false,
        }
    }
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

fn print_table_header<W: std::io::Write>(
    mut w: W,
    props: & [Property],
    desc: & str,
) -> std::io::Result<()> {
    let leftmost_column_width = 60;
    let rest_column_width = 15;

    write!(w, "{}", desc)?;
    writeln!(w, "")?;

    write!(w, " {:leftmost_column_width$} ", "controller")?;

    for prop in props {
        write!(w, "| {:rest_column_width$} ", prop.to_string())?;
        write!(w, "| {:rest_column_width$} ", "atime")?;
        write!(w, "| {:rest_column_width$} ", "vtime")?;
    }
    writeln!(w, "| {:rest_column_width$} ", "conforms")?;

    // dividing line
    write!(w, "-{:-<leftmost_column_width$}-", "")?;
    for _ in 0..(props.len() * 3) {
        write!(w, "+-{:-<rest_column_width$}-", "")?
    }
    writeln!(w, "+-{:-<rest_column_width$}-", "")?;
    Ok(())
}

fn print_ctrler_results<W: std::io::Write>(
    mut w: W,
    ctrler: &str,
    results: Vec<RunResult>,
) -> std::io::Result<()> {
    let leftmost_column_width = 60;
    let rest_column_width = 15;

    write!(w, " {:leftmost_column_width$} ", ctrler)?;

    for result in results.clone().into_iter() {
        write!(w, "| {:^rest_column_width$} ", result.error)?;
        write!(
            w,
            "| {:^rest_column_width$} ",
            format!("{:?}", result.analyze_time)
        )?;
        write!(
            w,
            "| {:^rest_column_width$} ",
            format!("{:?}", result.verify_time)
        )?;
        write!(w, "| {:^rest_column_width$} ", result.conforms_emoji())?;
    }

    // dividing line
    writeln!(w, "")?;
    write!(w, "-{:-<leftmost_column_width$}-", "")?;
    for _ in 0..(results.len() * 3) {
        write!(w, "+-{:-<rest_column_width$}-", "")?;
    }
    writeln!(w, "+-{:-<rest_column_width$}-", "")?;
    Ok(())
}

// runs given batch of controllers on given props
fn run_batch(
    args: &Args,
    batch: &[impl AsRef<str>],
    features: &[impl AsRef<str>],
    props: &[Property],
    desc: &str,
    expect_failure: bool,
) {
    use std::process::*;

    // for each ctrler, run dfpp (1) and test each property (props.len)
    let num_tasks = batch.len() * (1 + props.len());

    let progress = ProgressBar::new(num_tasks as u64).with_style(
        indicatif::ProgressStyle::default_bar()
            .template("{msg:11} {bar:40} {pos:>3}/{len:3}")
            .unwrap(),
    );

    let mut w = std::io::stdout();
    let verbose_commands = args.verbose_commands();

    print_table_header(&mut w, props, desc).unwrap();
    let ref lemmy_dir = std::env::current_dir()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned();

    for ctrler in batch {
        let mut ana_cmd = Command::new("cargo");
        ana_cmd.arg("paralegal-flow").current_dir(lemmy_dir).args([
            "--abort-after-analysis",
            "--target",
            "lemmy_api",
            "--external-annotations",
            "external-annotations.toml",
            "--",
            "--features",
            ctrler.as_ref(),
        ]);
        for feature in features {
            ana_cmd.args(["--features", feature.as_ref()]);
        }

        if !args.verbose {
            ana_cmd.stderr(Stdio::null()).stdout(Stdio::null());
        }
        if verbose_commands {
            progress.suspend(|| println!("Executing compile command: {:?}", ana_cmd));
        }
        let now = SystemTime::now();
        let ana_status = ana_cmd.status().unwrap();
        let analyze_time = now.elapsed().unwrap();
        progress.inc(1);

        let results = props
            .iter()
            .map(|typ| {
                let verify_time: Duration;
                let error = if !ana_status.success() {
                    verify_time = Duration::ZERO;
                    RunError::CompilationError
                } else {
                    let mut cmd = Command::new("cargo");
                    cmd.current_dir(&args.lemmy_prop_dir)
                        .arg("run")
                        .arg("--release")
                        .arg("--")
                        .arg(lemmy_dir)
                        .args(["--skip-compile", "--quiet"]);
                    cmd.arg("--prop");
                    cmd.arg(format!("{typ}"));

                    if !args.verbose {
                        cmd.stderr(Stdio::null()).stdout(Stdio::null());
                    }
                    if verbose_commands {
                        progress.suspend(|| println!("Executing check command: {:?}", cmd));
                    }
                    let now = SystemTime::now();
                    let status = cmd.status().unwrap();
                    verify_time = now.elapsed().unwrap();
                    if status.success() {
                        RunError::Success
                    } else {
                        RunError::CheckError
                    }
                };
                progress.inc(1);

                RunResult {
                    analyze_time,
                    verify_time,
                    error,
                    expected_outcome: !expect_failure,
                }
            })
            .collect::<Vec<_>>();

        progress.suspend(|| print_ctrler_results(&mut w, ctrler.as_ref(), results).unwrap());
    }

    progress.finish_and_clear();
}

impl BatchConfig<'_> {
    fn run(&self, args: &Args) {
        let initial_batches = self.baseline_controllers.iter().cloned().chain(
            self.change
                .as_ref()
                .map(|c| c.affected_controllers)
                .flatten(),
        );

        let mut features = vec![self.baseline_feature];
        if let Some(change) = self.change.as_ref() {
            if !change.add_feature {
                features.push(change.change_feature);
            }
        }
        let props = [self.property];
        let expect_failure = self.expect_failure;

        for (batch_num, batch) in initial_batches.enumerate() {
            let desc = format!("Initial batch {batch_num} {}", self.comment.unwrap_or(""));
            run_batch(args, batch, &features, &props, &desc, expect_failure);
        }

        if let Some(change) = self.change.as_ref() {
            let second_batches = if change.affected_controllers.is_some() {
                change.affected_controllers.as_slice()
            } else {
                self.baseline_controllers
            };

            let mut features = vec![self.baseline_feature];
            if change.add_feature {
                features.push(change.change_feature);
            }

            for (batch_num, batch) in second_batches.iter().copied().enumerate() {
                let desc = format!("Changed batch {batch_num} {}", self.comment.unwrap_or(""));
                run_batch(args, batch, &features, &props, &desc, !expect_failure);
            }
        }
    }
}

// Runs controllers relevant for each bug (1-4)
// For Bug 1, this is all of the controllers twice: once before the bug fix, once after
// For Bug 2, this is login twice: once before the bug fix, once after
// For Bug 3, this is two batches: the controllers the Lemmy developers found, and the ones Paralegal found.
// For the controllers that the Lemmy developers found, each controller runs once before the bug fix, once after
// For Bug 4, this is once batch: the controllers Paralegal found
fn main() {
    let args = Args::parse();

    if args.ctrlers.is_empty() {
        println!("INFO: No controllers specified; running relevant controllers and properties for each bug");
        let bugs = if !args.bug.is_empty() {
            args.bug.clone()
        } else {
            GetUserVersion::value_variants().to_vec()
        };
        for bug in bugs {
            bug.to_config().run(&args)
        }
    } else {
        println!("INFO: Running specified controllers. The `conforms` column is meaningless here");
        assert!(args.bug.is_empty(), "the --bug flag is ignored when --ctrlers is specified");
        let features: &[&str] = &[];
        run_batch(&args, &args.ctrlers, features, CONFIGURATIONS, "", true);
    }
}
