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
    // "user-list-banned",
    // "user-login",
    // "user-login-buggy",
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
    // "comment-create-buggy",
    // "comment-delete",
    // "comment-list",
    // "comment-read",
    // "comment-remove",
    // "comment-update",
    // "comment-update-buggy",
    // "comment-create",
    // "community-delete",
    // "community-list",
    // "post-read",
    // "community-remove",
    // "community-update",
    // "post-create",
    // "post-create-buggy",
    // "post-delete",
    // "post-delete-buggy",
    // "post-list",
    // "post-read",
    // "post-remove",
    // "post-update",
    // "post-update-buggy",
    // "private-message-create",
    // "private-message-delete",
    // "private-message-read",
    // "private-message-update",
    // "site-create",
    // "site-read",
    // "site-update",
    // "user-create",
    // "user-delete",
    // "user-read"
];

// const SHOULD_FAIL_READ &'static [&'static str] = &[
//     "user-create"
// ];

// const SHOULD_FAIL_WRITE &'static [&'static str] = &[
//     "comment-like",
//     "comment-mark-as-read",
//     "comment-save",
//     "comment-report-create",
//     "community-block",
//     "user-login-buggy",
//     "post-mark-read",
//     "post-save",
//     "post-report-create",
//     "post-report-resolve",
//     "comment-create-buggy",
//     "comment-delete",
//     "comment-remove",
//     "comment-update-buggy",
//     "community-delete",
//     "post-read",
//     "community-remove",
//     "community-update",
//     "post-create-buggy",
//     "post-delete-buggy",
//     "post-remove",
//     "post-update-buggy"
// ];

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
            let mut external_ann_file: std::pa#![feature(scoped_threads)]

            extern crate clap;
            extern crate indicatif;
            extern crate humantime;
            use clap::Parser;
            
            use indicatif::ProgressBar;
            use either::Either;
            
            use std::collections::{HashSet, HashMap};
            use std::fmt::{Display, Write};
            use std::str::FromStr;
            use std::sync::{mpsc::channel, Arc, Mutex};
            use std::time::{Duration, SystemTime};
            
            const CONFIGURATIONS: &'static [Property] = &[
                Property::Instance,
                Property::Community,
            ];
            
            const PROPS_PATH: &str = "../../dfpp-props/";
            
            const ERR_MSG_VERSIONS: &[&str] = &["original", "optimized", "minimal"];
            
            const ALL_KNOWN_CTRLERS: &'static [&'static str] = &[
                "comment-like",
                "comment-mark-as-read",
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
                // "user-login",
                // "user-login-buggy",
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
                // "comment-create-buggy",
                // "comment-delete",
                // "comment-list",
                // "comment-read",
                // "comment-remove",
                // "comment-update",
                // "comment-update-buggy",
                // "comment-create",
                // "community-delete",
                // "community-list",
                // "post-read",
                // "community-remove",
                // "community-update",
                // "post-create",
                // "post-create-buggy",
                // "post-delete",
                // "post-delete-buggy",
                // "post-list",
                // "post-read",
                // "post-remove",
                // "post-update",
                // "post-update-buggy",
                // "private-message-create",
                // "private-message-delete",
                // "private-message-read",
                // "private-message-update",
                // "site-create",
                // "site-read",
                // "site-update",
                // "user-create",
                // "user-delete",
                // "user-read"
            ];
            
            // const SHOULD_FAIL_READ &'static [&'static str] = &[
            //     "user-create"
            // ];
            
            // const SHOULD_FAIL_WRITE &'static [&'static str] = &[
            //     "comment-like",
            //     "comment-mark-as-read",
            //     "comment-save",
            //     "comment-report-create",
            //     "community-block",
            //     "user-login-buggy",
            //     "post-mark-read",
            //     "post-save",
            //     "post-report-create",
            //     "post-report-resolve",
            //     "comment-create-buggy",
            //     "comment-delete",
            //     "comment-remove",
            //     "comment-update-buggy",
            //     "community-delete",
            //     "post-read",
            //     "community-remove",
            //     "community-update",
            //     "post-create-buggy",
            //     "post-delete-buggy",
            //     "post-remove",
            //     "post-update-buggy"
            // ];
            
            /// Batch executor for the evaluation of our 2023 Eurosys paper.
            ///
            /// Be aware that this tool does not install dfpp itself but assumes the latest
            /// version is already present and in the $PATH.
            #[derive(Parser)]
            struct Args {
                /// Print complete error messages for called programs on failure (implies
                /// `--verbose-commands`)
                #[clap(long, default_value = "true")]
                verbose: bool,
            
                /// Print the shell commands we are running
                #[clap(long)]
                verbose_commands: bool,
            
                /// Controllers to run in api
                ctrlers: Vec<String>,
            
                /// Location of the Lemmy repo
                #[clap(long, default_value = "..")]
                directory: std::path::PathBuf,
            
                #[clap(long, default_value = "verification")]
                output_directory: std::path::PathBuf,
            
                #[clap(long, default_value = "props")]
                forge_source_dir: std::path::PathBuf,
            
                #[clap(long, default_value = "1h")]
                err_msg_timeout: humantime::Duration,
            
                #[clap(long, default_value = "10m")]
                check_timeout: humantime::Duration,
            
                /// Error message version to run. Options: "original", "minimal",
                /// "optimized", default to all
                #[clap(long = "emv")]
                error_message_versions: Option<Vec<String>>,
            
                #[clap(long, default_value_t = 4)]
                parallelism: usize,
            }
            
            impl Args {
                fn verbose_commands(&self) -> bool {
                    self.verbose || self.verbose_commands
                }
            
                fn error_message_versions(&self) -> bool {
                    self.error_message_versions
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
            enum RunResult {
                Success(Duration),
                CompilationError,
                CheckError(Duration),
                Timeout,
            }
            
            impl std::fmt::Display for RunResult {
                fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    use std::fmt::Alignment;
                    let width = formatter.width().unwrap_or(2);
                    let selfstr = match self {
                        RunResult::Success(dur) => format!("✅ ({})", humantime::format_duration(*dur)),
                        RunResult::CompilationError => "️🚧".to_string(),
                        RunResult::CheckError(dur) => format!("❌ ({})", humantime::format_duration(*dur)),
                        RunResult::Timeout => "⏲".to_string(),
                    };
                    let selfwidth = selfstr.len();
                    let (before, after) = match formatter.align() {
                        None => (0, width - selfwidth),
                        _ if width < selfwidth => (0, 0),
                        Some(Alignment::Left) => (0, width - selfwidth),
                        Some(Alignment::Right) => (width - selfwidth, 0),
                        Some(Alignment::Center) => {
                            let left = (width - selfwidth) / 2;
                            (left, width - selfwidth - left)
                        }
                    };
                    let fill_chr = formatter.fill();
                    for _ in 0..before {
                        formatter.write_char(fill_chr)?;
                    }
                    formatter.write_str(&selfstr)?;
                    for _ in 0..after {
                        formatter.write_char(fill_chr)?;
                    }
                    Ok(())
                }
            }
            
            #[derive(Debug)]
            struct StringErr(String);
            
            impl std::fmt::Display for StringErr {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str(&self.0)
                }
            }
            
            impl std::error::Error for StringErr {}
            
            fn read_and_count_forge_unsat_instance(all: &str) -> Result<u32, String> {
                extern crate serde_lexpr as sexpr;
                use std::io::Read;
                let target = all
                    .split_once("'(#hash")
                    .ok_or("Did not find pattern \"'(#hash\"")?
                    .1;
                let target = target
                    .rsplit_once("'((")
                    .ok_or("Did not find pattern \"'((\" at the file end")?
                    .0;
                let target = target
                    .rsplit_once(")")
                    .ok_or("Did not find pattern \")\" before \"'((\" at the file end")?
                    .0;
                let value = sexpr::parse::from_str(target).map_err(|e| e.to_string())?;
                let flow = value
                    .get("minimal_subflow")
                    .ok_or("Did not find 'minimal_subflow' key")?;
                Ok(flow
                    .list_iter()
                    .ok_or("'minimal_subflow' is not an s-expression list")?
                    .map(|v| {
                        match v
                            .to_ref_vec()
                            .ok_or("'minimal_subflow' elements are not lists")?
                            .as_slice()
                        {
                            [_, from, to] => Ok((
                                from.as_symbol().ok_or(
                                    "Second elements of 'minimal_subflow' elements should be a symbol",
                                )?,
                                to.as_symbol()
                                    .ok_or("Third elements of 'minimal_subflow' elements should be a symbol")?,
                                0,
                            )),
                            _ => Err("'minimal_subflow' list elements should be 3-tuples"),
                        }
                    })
                    .count() as u32)
            }
            
            #[derive(Clone, Copy)]
            enum ErrMsgResult {
                Timeout,
                Success(std::time::Duration, u32),
                Sat(std::time::Duration),
            }
            
            impl std::fmt::Display for ErrMsgResult {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    match self {
                        ErrMsgResult::Timeout => f.write_str("timed out"),
                        ErrMsgResult::Success(time, edgecount) => write!(
                            f,
                            "succeeded in {} with a graph of {edgecount}",
                            humantime::format_duration(*time)
                        ),
                        ErrMsgResult::Sat(duration) => write!(
                            f,
                            "was satisfiable in {}",
                            humantime::format_duration(*duration)
                        ),
                    }
                }
            }
            
            fn with_timeout<R: std::marker::Send + 'static, F: FnOnce() -> R + std::marker::Send + 'static>(timeout: Duration, f: F) -> Option<R> {
                use std::sync::mpsc::{channel};
                use std::thread;
            
                let (send, rcv) = channel();
            
                std::thread::spawn(move || {
                    send.send(f())
                });
            
                rcv.recv_timeout(timeout).ok()
            }
            
            struct RunConfiguration {
                typ: Property,
                ctrler: &'static String,
                progress: &'static ProgressBar,
                args: &'static Args,
            }
            
            impl RunConfiguration {
                fn describe(&self) -> String {
                    use std::fmt::Write;
                    let mut s = String::new();
                    write!(s, "{}-{}", self.typ, self.ctrler).unwrap();
                    s
                }
            
                fn props_path(&self) -> &std::path::Path {
                    std::path::Path::new(PROPS_PATH)
                }
            
                fn err_msg_timeout(&self) -> std::time::Duration {
                    self.args.err_msg_timeout.into()
                }
                fn check_timeout(&self) -> std::time::Duration {
                    self.args.check_timeout.into()
                }
                fn forge_source_dir(&self) -> &std::path::Path {
                    self.args.forge_source_dir.as_path()
                }
                fn verbose(&self) -> bool {
                    self.args.verbose
                }
                fn verbose_commands(&self) -> bool {
                    self.args.verbose_commands()
                }
                fn outpath(&self) -> std::path::PathBuf {
                    let edit =  "original".to_string();
                    self.args.output_directory.join(edit)
                }
                fn forge_file_name_for(&self, what: &str) -> String {
                    format!("{}-{what}-{}.frg", self.typ, self.ctrler)
                }
                fn forge_in_file(&self, what: &str) -> std::path::PathBuf {
                    self.forge_source_dir().join(self.forge_file_name_for(what))
                }
                fn forge_out_file(&self, what: &str) -> std::path::PathBuf {
                    self.outpath().join(self.forge_file_name_for(what))
                }
                fn analysis_result_path(&self) -> std::path::PathBuf {
                    self.forge_out_file("analysis-result")
                }
            
                fn compile(&self) -> anyhow::Result<bool> {
                    use std::process::*;
                    let result_file_path = self.analysis_result_path();
                    let mut dfpp_cmd = Command::new("cargo");
                    dfpp_cmd
                        .args([
                            "dfpp",
                            "--result-path",
                            &result_file_path.to_string_lossy(),
                            "--model-version",
                            "v2",
                            "--target",
                            "lemmy_api",
                            "--inline-elision",
                            "--skip-sigs",
                            "--abort-after-analysis",
                            "--external-annotations",
                            "external-annotations.toml"
                        ])
                        .stdin(Stdio::null());
            
                    let external_ann_file_name = format!("external-annotations.toml");
                    let mut external_ann_file: std::path::PathBuf = self.args.directory.clone().into();
                    external_ann_file.push(&external_ann_file_name);
                    if external_ann_file.exists() {
                        dfpp_cmd.args(&["--external-annotations", external_ann_file_name.as_str()]);
                    }
                    let ctrler = self.ctrler.to_owned();
                    dfpp_cmd.args(&["--", "--features", &format!("{ctrler}")]);
                    if !self.verbose() {
                        dfpp_cmd.stderr(Stdio::null()).stdout(Stdio::null());
                    }
                    if self.verbose_commands() {
                        self.progress.suspend(|| println!("Executing compile command: {:?}", dfpp_cmd));
                    }
                    let mut now = SystemTime::now();
                    let status = dfpp_cmd.status().unwrap();
                    let analyze_time = now.elapsed().unwrap();
                    self.progress.inc(1);
                    // if !status.success() {
                    //     Ok(false)
                    // } // NOTE: This is commented out because `cargo dfpp` always returns error for some reason, but it works (usually).
                    Ok(true)
                }
            
                fn run_prop(&self) -> anyhow::Result<RunResult> {
                    use std::process::*;
            
                    let now = std::time::Instant::now();
                    let check_file_path = self.forge_out_file("check");
                    {
                        use std::io::{Read, Write};
                        let mut w = std::fs::OpenOptions::new()
                            .truncate(true)
                            .write(true)
                            .create(true)
                            .open(&check_file_path)?;
                        self.write_headers_and_prop(&mut w, &self.props_path().join("sigs"))?;
                        writeln!(
                            w,
                            "test expect {{ {}: {{ property[flow, labels] }} for Flows is theorem }}",
                            self.typ
                        )?;
                    }
                    let mut racket_cmd = Command::new("racket");
                    racket_cmd
                        .arg(&check_file_path)
                        .stdin(Stdio::null())
                        .stdout(Stdio::piped());
                    if !self.verbose() {
                        racket_cmd.stderr(Stdio::null()).stdout(Stdio::null());
                    }
                    if self.verbose_commands() {
                        self.progress
                            .suspend(|| println!("Executing check command: {:?}", racket_cmd));
                    }
                    with_timeout(self.check_timeout(), move || racket_cmd.status()).map_or(Ok(RunResult::Timeout), |status| {
                        self.progress.inc(1);
                        if status?.success() {
                            Ok(RunResult::Success(now.elapsed()))
                        } else {
                            Ok(RunResult::CheckError(now.elapsed()))
                        }
                    })
                }
            
                fn run_error_msg(&self, template: &str) -> anyhow::Result<ErrMsgResult> {
                    use std::process::*;
                    let frg_file = self.forge_out_file(&format!("err-msg-check-{template}"));
                    {
                        use std::io::{copy, Read, Write};
                        let mut w = std::fs::OpenOptions::new()
                            .truncate(true)
                            .write(true)
                            .create(true)
                            .open(&frg_file)?;
                        let sig_file = if template == "optimized" {
                            self.props_path().join("err_msg_optimized_sigs")
                        } else {
                            self.props_path().join("err_msg_sigs")
                        };
                        self.write_headers_and_prop(&mut w, &sig_file)?;
                        let template_file = self
                            .forge_source_dir()
                            .join(self.props_path().join(format!("err_msg_template_{template}.frg")));
                        copy(&mut std::fs::File::open(template_file)?, &mut w)?;
                    }
                    let mut racket_cmd = Command::new("racket");
                    racket_cmd.arg(&frg_file).stdin(Stdio::null());
                    if !self.verbose() {
                        racket_cmd.stderr(Stdio::null()).stdout(Stdio::null());
                    }
                    if self.verbose_commands() {
                        self.progress
                            .suspend(|| println!("Executing check command: {:?}", racket_cmd));
                    }
                    let time = std::time::Instant::now();
                    let child = racket_cmd.spawn()?;
            
                    let output = with_timeout(self.err_msg_timeout(), || {
                        child.wait_with_output()
                    });
            
                    if let Some(output) = output {
                        let output = output?;
                        if output.status.success() {
                            Ok(ErrMsgResult::Sat(time.elapsed()))
                        } else {
                            let forge_output_str = String::from_utf8_lossy(&output.stdout);
                            let counting_tesult = read_and_count_forge_unsat_instance(&forge_output_str);
                            if counting_tesult.is_err() {
                                use std::io::Write;
                                write!(
                                    &mut std::fs::OpenOptions::new()
                                        .create(true)
                                        .truncate(true)
                                        .write(true)
                                        .open(self.args.output_directory.join("err_msg_output.txt"))?,
                                    "{}",
                                    forge_output_str,
                                )?;
                            }
                            Ok(ErrMsgResult::Success(
                                time.elapsed(),
                                counting_tesult.map_err(StringErr)?,
                            ))
                        }
                    } else {
                         Ok(ErrMsgResult::Timeout)
                    }
                }
            
            
                fn write_headers_and_prop<W: std::io::Write, P: AsRef<std::path::Path>>(
                    &self,
                    mut w: W,
                    sigs: &P,
                ) -> std::io::Result<()> {
                    use std::io::{copy, Read, Write};
                    let propfile = self.forge_in_file("props");
                    writeln!(w, "#lang forge")?;
                    let ana_path = self.analysis_result_path();
                    use Either::*;
                    let helper_files = [self.props_path().join("basic-helpers")];
                    for include in [Right(sigs.as_ref().into()), Left(ana_path)]
                        .into_iter()
                        .chain(helper_files.into_iter().map(Right))
                    {
                        writeln!(w)?;
                        let path = match include {
                            Right(include) => self.forge_source_dir().join(include).with_extension("frg"),
                            Left(path) => path,
                        };
                        writeln!(w, "// {}", path.display())?;
                        copy(&mut std::fs::File::open(path)?, &mut w)?;
                    }
                    copy(&mut std::fs::File::open(propfile)?, &mut w)?;
                    Ok(())
                }
            }
            
            type ResultTable = HashMap<
                Property,
                HashMap<
                    &'static str,
                    (
                        RunConfiguration,
                        Mutex<(Option<RunResult>, Vec<(&'static str, ErrMsgResult)>)>,
                    ),
                >,
            >;
            
            fn print_results_for_property<W: std::io::Write>(
                mut w: W,
                num_versions: usize,
                args: &Args,
                results: &ResultTable,
            ) -> std::io::Result<()> {
                let head_cell_width = 5;
                let body_cell_width = 20;
            
                for (typ, results) in results.iter() {
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
            
                    let mut edits = results.iter().collect::<Vec<_>>();
                    edits.sort_by_key(|e| e.0);
                    for (i, (_, (_, mutex))) in edits.iter().enumerate() {
                        let result = mutex.try_lock().unwrap();
                        let run_result = result.0.unwrap();
                        write!(w, "| {:^body_cell_width$} ", run_result)?;
                    }
                    writeln!(w, "")?;
                }
                writeln!(w, "")
            }
            
            fn main() {
                use std::io::Write;
                let args = Box::leak::<'static>(Box::new(Args::parse()));
                if args.ctrlers.is_empty() {
                    println!("INFO: No specification variants to run given, running all known ones");
                    args.ctrlers = ALL_KNOWN_CTRLERS
                        .iter()
                        .cloned()
                        .map(str::to_string)
                        .collect();
                }
                std::env::set_current_dir(&args.directory);
                assert!(args.parallelism > 0);
            
                let error_message_versions: Vec<_> = if let Some(v) = args.error_message_versions().as_ref() {
                    let str_refs = v.iter().map(String::as_str).collect::<Vec<_>>();
                    if let ["none"] = str_refs.as_slice() {
                        vec![]
                    } else {
                        str_refs
                    }
                } else {
                    ERR_MSG_VERSIONS.to_vec()
                };
            
                let num_versions = args.ctrlers.len();
            
                let num_configurations = CONFIGURATIONS
                    .len()
                    * (2 // compile 
                        * num_versions
                        + num_versions * error_message_versions.len());
            
                let mut progress = Box::leak::<'static>(Box::new(
                    ProgressBar::new(num_configurations as u64).with_style(
                        indicatif::ProgressStyle::default_bar()
                            .template("{msg:11} {bar:40} {pos:>3}/{len:3}")
                            .unwrap(),
                    ),
                ));
            
                let mut w = std::io::stdout();
                let mut dir_builder = std::fs::DirBuilder::new();
                dir_builder.recursive(true);
                let results: ResultTable = CONFIGURATIONS
                    .into_iter()
                    .map(|&typ| {
                        (
                            typ,
                            args.ctrlers
                                .iter()
                                .map(|ctrler| {
                                    let config = RunConfiguration {
                                        typ,
                                        ctrler,
                                        progress,
                                        args,
                                    };
                                    let outpath = config.outpath();
                                    if !outpath.exists() {
                                        dir_builder.create(outpath).unwrap();
                                    }
                                    assert!(config.compile().unwrap());
                                    (ctrler.as_str(), (config, Mutex::new((None, vec![]))))
                                })
                                .collect(),
                        )
                    })
                    .collect();
            
                std::thread::scope(|scope| {
                    let (send_work, receive_work) = channel();
            
                    for t in results.values() {
                        for descr in t.values() {
                            send_work.send(descr).unwrap()
                        }
                    }
            
                    let receive_work = Arc::new(Mutex::new(receive_work));
            
                    for _ in 0..args.parallelism {
                        let my_receive = receive_work.clone();
                        let my_results_ref = &results;
                        std::thread::Builder::new()
                            .spawn_scoped(scope, move || {
                                while let Some((config, mutex)) =
                                    my_receive.lock().ok().and_then(|r| r.recv().ok())
                                {
                                    let mut guard = mutex.try_lock().unwrap();
                                    assert!(guard.0.replace(config.run_prop().unwrap()).is_none());
                                }
                            })
                            .unwrap();
                    }
                });
            
                print_results_for_property(
                    &mut w,
                    num_versions,
                    &args,
                    &results,
                )
                .unwrap();
                writeln!(w, "Error message results:").unwrap();
            
                std::thread::scope(|scope| {
                    let (send_work, receive_work) = channel();
            
                    for t in results.values() {
                        for (config, result_mutex) in t.values() {
                            if matches!(
                                result_mutex.try_lock().unwrap().0.unwrap(),
                                RunResult::CheckError(_)
                            ) {
                                for emv in error_message_versions.iter() {
                                    send_work.send((config, result_mutex, emv)).unwrap()
                                }
                            } else {
                                progress.inc(error_message_versions.len() as u64);
                            }
                        }
                    }
            
                    let receive_work = Arc::new(Mutex::new(receive_work));
            
                    for _ in 0..args.parallelism {
                        let my_receive = receive_work.clone();
                        let my_results_ref = &results;
                        let progress_ref = &progress;
                        std::thread::Builder::new()
                            .spawn_scoped(scope, move || {
                                while let Some((config, mutex, emv)) =
                                    my_receive.lock().ok().and_then(|r| r.recv().ok())
                                {
                                    let emvresult = config.run_error_msg(emv).unwrap();
                                    progress_ref.inc(1);
                                    mutex.lock().unwrap().1.push((emv, emvresult));
                                }
                            })
                            .unwrap();
                    }
                });
            
                for type_results in results.values() {
                    for (config, result) in type_results.values() {
                        let results = &result.try_lock().unwrap();
                        if matches!(results.0, Some(RunResult::CheckError(_))) {
                            for (emv, result) in results.1.iter() {
                                progress.suspend(|| {
                                    writeln!(w, "{}: {emv} {result}", config.describe()).unwrap();
                                });
                            }
                        } 
                    }
                }
                progress.finish_and_clear();
            }
            th::PathBuf = cd.into();
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
    let head_cell_width = 5;
    let body_cell_width = 20;

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