use clap::{Args, ValueEnum};
use paralegal_policy::{assert_warning, Config, GraphLocation};
use std::borrow::Cow;
use std::fmt::Write;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::Prop;

pub const LEMMY_API_CONTROLLERS: &[&str] = &[
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
];

pub const LEMMY_API_CRUD_CONTROLLERS: &[&str] = &[
    "comment-create",
    "comment-delete",
    "comment-list",
    "comment-read",
    "comment-remove",
    "comment-update",
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
    "site-read",
    "site-update",
    "user-create",
    "user-delete",
    "user-read",
];

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
    "comment-create",
    "comment-delete",
    "comment-list",
    "comment-read",
    "comment-remove",
    "comment-update",
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
    "site-read",
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
    "comment-create",
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
    "site-read",
    "site-update",
    "user-delete",
    "user-read",
];

pub struct BatchConfig<'a> {
    pub baseline_feature: &'a str,
    pub expect_failure: bool,
    pub property: Prop,
    pub description: &'a str,
    pub baseline_controllers: &'a [&'a [&'a str]],
    pub change: Option<Change<'a>>,
}

pub struct Change<'a> {
    pub change_feature: &'a str,
    pub add_feature: bool,
    /// If this is `None`, all baseline controllers are affected.
    ///
    /// If this is `Some` it will contain a list of controllers that are changed with this feaure.
    /// That list of controllers should *not* be contained in the `baseline_controllers`
    /// field in [`BatchConfig`].
    pub affected_controllers: Option<&'a [&'a str]>,
}

const BUG_1_CONFIG: BatchConfig<'static> = BatchConfig {
    baseline_feature: "bug-1-code",
    expect_failure: true,
    property: Prop::Instance,
    baseline_controllers: &[BUG_1_BATCH_1, BUG_1_BATCH_2, BUG_1_BATCH_3],
    description: "Bug 1 - initial missing instance delete/ban check",
    change: Some(Change {
        change_feature: "bug-1-fix",
        add_feature: true,
        affected_controllers: None,
    }),
};

const BUG_2_CONFIG: BatchConfig<'static> = BatchConfig {
    baseline_feature: "post-bug-1",
    expect_failure: false,
    property: Prop::Instance,
    baseline_controllers: &[POST_BUG_1_BATCH_1, POST_BUG_1_BATCH_2, POST_BUG_1_BATCH_3],
    description: "Bug 2 - Refactoring for instance ban/delete checks",
    change: Some(Change {
        change_feature: "correct",
        add_feature: false,
        affected_controllers: Some(&["user-login"]),
    }),
};

const BUG_3_CONFIG: BatchConfig<'static> = BatchConfig {
    baseline_feature: "post-bug-1",
    expect_failure: true,
    property: Prop::Community,
    baseline_controllers: &[BUG_3_BATCH],
    description: "Bug 3 - Missing community ban/delete checks that the lemmy developers fixed",
    change: Some(Change {
        change_feature: "correct",
        add_feature: true,
        affected_controllers: Some(BUG_3_FIXED_BATCH),
    }),
};

const BUG_4_CONFIG: BatchConfig<'static> = BatchConfig {
    baseline_feature: "post-bug-1",
    expect_failure: true,
    property: Prop::Community,
    description: "Bug 4 - Additional missing community man/delete checks that Paralegal found",
    baseline_controllers: &[],
    change: Some(Change {
        change_feature: "correct,hypothetical-fix",
        add_feature: true,
        affected_controllers: Some(BUG_4_BATCH),
    }),
};

// Bug 1 all fail
// Bug fix all pass
// Bug 2 succeeds with "correct"
// Bug 3 one of them succeds with "correct"
// Bug 3 the other one always fails
// Bug 4 always fails

// used to all have "post-bug-1" prepended
// these are the buggy controllers that the Lemmy developers found and fixed themselves
const BUG_3_FIXED_BATCH: &'static [&'static str] = &[
    "comment-create",
    "comment-update",
    "post-create",
    "post-delete",
    "post-update",
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

#[derive(
    Clone,
    Copy,
    Eq,
    PartialEq,
    Hash,
    strum::Display,
    clap::ValueEnum,
    Debug,
    serde::Deserialize,
    serde::Serialize,
    strum::AsRefStr,
)]
#[clap(rename_all = "kebab-case")]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum GetUserVersion {
    Bug1,
    Bug2,
    Bug3,
    Bug4,
}

impl GetUserVersion {
    pub fn to_config(self) -> &'static BatchConfig<'static> {
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
    props: &[Prop],
    desc: &str,
) -> std::io::Result<()> {
    let leftmost_column_width = 60;
    let rest_column_width = 15;

    write!(w, "{}", desc)?;
    writeln!(w, "")?;

    write!(w, " {:leftmost_column_width$} ", "controller")?;

    for prop in props {
        write!(w, "| {:rest_column_width$} ", prop.as_ref())?;
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

#[derive(Clone, Copy, ValueEnum, serde::Deserialize, serde::Serialize)]
pub enum LemmyPackage {
    Api,
    ApiCrud,
}

impl LemmyPackage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Api => "lemmy_api",
            Self::ApiCrud => "lemmy_api_crud",
        }
    }
}

// runs given batch of controllers on given props
fn run_batch(
    common_args: &CommonArgs,
    batch: impl IntoIterator<Item = impl AsRef<str>>,
    features: &[impl AsRef<str>],
    props: &[Prop],
    desc: &str,
    expect_failure: bool,
    package: LemmyPackage,
) -> bool {
    use std::process::*;
    let mut w = std::io::stdout();

    let mut failed = false;

    print_table_header(&mut w, props, desc).unwrap();

    for ctrler in batch {
        let mut ana_cmd = Command::new("cargo");
        ana_cmd
            .arg("paralegal-flow")
            .current_dir(&common_args.path)
            .args([
                "--abort-after-analysis",
                "--target",
                package.as_str(),
                "--external-annotations",
                "external-annotations.toml",
                "--relaxed",
                "--",
                "--features",
                ctrler.as_ref(),
            ]);
        for feature in features {
            ana_cmd.args(["--features", feature.as_ref()]);
        }

        ana_cmd.stdout(Stdio::null()).stderr(Stdio::null());

        let now = SystemTime::now();
        let ana_status = ana_cmd.status().unwrap();
        let analyze_time = now.elapsed().unwrap();

        let results = props
            .iter()
            .map(|typ| {
                let verify_time: Duration;
                let error = if !ana_status.success() {
                    verify_time = Duration::ZERO;
                    RunError::CompilationError
                } else {
                    let now = SystemTime::now();
                    let graph_file = paralegal_policy::GraphLocation::std(&common_args.path);
                    let status =
                        run_policies_for_props(graph_file, &[*typ], common_args.new, true, false);

                    verify_time = now.elapsed().unwrap();
                    match status {
                        Ok(true) => RunError::Success,
                        _ => {
                            failed = true;
                            RunError::CheckError
                        }
                    }
                };

                RunResult {
                    analyze_time,
                    verify_time,
                    error,
                    expected_outcome: !expect_failure,
                }
            })
            .collect::<Vec<_>>();

        print_ctrler_results(&mut w, ctrler.as_ref(), results).unwrap();
    }
    !failed
}

fn resplit_batches_for_package<'a: 'b, 'b>(
    iter: impl IntoIterator<Item = &'b [&'a str]>,
) -> (Vec<&'a str>, Vec<&'a str>) {
    iter.into_iter()
        .flatten()
        .partition(|c| LEMMY_API_CONTROLLERS.contains(c))
}

/// This unifies handling for new and old batches. In the old version we
/// basically just return the input iterator, but in the new version we return
/// it split by the package that it runs in, e.g. api vs api_crud
fn split_batches_if_new<'a: 'b, 'b>(
    initial_batches: impl IntoIterator<Item = &'b [&'a str]> + 'b,
    new: bool,
) -> Box<dyn Iterator<Item = (Cow<'static, str>, LemmyPackage, Cow<'b, [&'a str]>)> + 'b> {
    if new {
        let (api, api_crud) = resplit_batches_for_package(initial_batches);
        Box::new(
            [
                (Cow::Borrowed("api"), LemmyPackage::Api, Cow::Owned(api)),
                (
                    Cow::Borrowed("api_crud"),
                    LemmyPackage::ApiCrud,
                    Cow::Owned(api_crud),
                ),
            ]
            .into_iter(),
        ) as Box<dyn Iterator<Item = _>>
    } else {
        Box::new(initial_batches.into_iter().enumerate().map(|(i, b)| {
            (
                Cow::Owned(i.to_string()),
                LemmyPackage::Api,
                Cow::Borrowed(b),
            )
        }))
    }
}

impl BatchConfig<'_> {
    pub fn run(&self, common_args: &CommonArgs) -> bool {
        let mut failed = false;
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

        println!("### {} ###", self.description);

        let batches = split_batches_if_new(initial_batches, common_args.new);

        for (batch_num, package, batch) in batches {
            let desc = format!("Initial batch {batch_num}");
            run_batch(
                common_args,
                batch.as_ref(),
                &features,
                &props,
                &desc,
                expect_failure,
                package,
            );
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

            let batches = split_batches_if_new(second_batches.iter().copied(), common_args.new);

            for (batch_num, package, batch) in batches {
                let desc = format!("Changed batch {batch_num}");
                failed |= !run_batch(
                    common_args,
                    batch.as_ref(),
                    &features,
                    &props,
                    &desc,
                    !expect_failure,
                    package,
                );
            }
        }
        println!();
        !failed
    }
}

#[derive(Args)]
pub struct CommonArgs {
    #[clap(long)]
    new: bool,
    #[clap(long, default_value = "case-studies/lemmy")]
    pub path: PathBuf,
}

#[derive(Args)]
pub struct SelectionArgs {
    /// Property selection. If none are selected, all are run
    #[clap(long)]
    pub prop: Vec<Prop>,
    /// Controller selectoin. If none are selected, all are run
    #[clap(long, short)]
    pub controller: Vec<String>,
    /// Don't run the PDG compiler. Assumes an artifact file is found in
    /// the standard location
    #[clap(long)]
    pub skip_compile: bool,

    #[clap(long, conflicts_with = "quiet")]
    pub verbose: bool,

    #[clap(long, short)]
    pub quiet: bool,

    #[clap(long)]
    package: LemmyPackage,

    /// Additional arguments to pass through to paralegal/cargo
    #[clap(last = true)]
    pub extra_args: Vec<String>,
}

fn run_policies_for_props(
    graph_file: GraphLocation,
    prop: &[Prop],
    new: bool,
    quiet: bool,
    verbose: bool,
) -> anyhow::Result<bool> {
    let config = Config::default();

    let cx = Arc::new(graph_file.build_context(config)?);
    assert_warning!(
        cx,
        !cx.desc().controllers.is_empty(),
        "No controllers found. Your policy is likely to be vacuous."
    );
    for p in if prop.is_empty() {
        Prop::value_variants()
    } else {
        prop
    } {
        p.run(cx.clone(), new, verbose)?;
    }

    let writer = if quiet {
        Box::new(std::io::sink()) as Box<dyn std::io::Write>
    } else {
        Box::new(std::io::stdout())
    };

    let success = cx.emit_diagnostics(writer)?;
    Ok(success)
}

impl SelectionArgs {
    pub fn run(&self, args: &CommonArgs) -> anyhow::Result<bool> {
        let all_controllers = ["all_controllers".to_owned()];
        let selected_controllers = if self.controller.is_empty() {
            &all_controllers as &[_]
        } else {
            &self.controller
        };
        let graph_file = if self.skip_compile {
            paralegal_policy::GraphLocation::std(&args.path)
        } else {
            let mut cmd = paralegal_policy::SPDGGenCommand::global();
            cmd.external_annotations("external-annotations.toml");
            cmd.abort_after_analysis();
            let rcmd = cmd.get_command();
            rcmd.arg("--target")
                .arg(self.package.as_str())
                .arg("--relaxed")
                .args(&self.extra_args);
            if self.quiet {
                rcmd.stdout(Stdio::null()).stdin(Stdio::null());
            }
            if !self.extra_args.contains(&"--".to_owned()) {
                rcmd.arg("--");
            }
            for c in selected_controllers {
                rcmd.args(["--features", &c]);
            }
            cmd.run(&args.path)?
        };

        run_policies_for_props(
            graph_file,
            self.prop.as_slice(),
            args.new,
            self.quiet,
            self.verbose,
        )
    }
}
