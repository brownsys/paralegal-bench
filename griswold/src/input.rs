//! Types describing data the runner ingests

use indexmap::IndexMap;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, time::Duration};

#[derive(Clone, Copy, PartialEq, Eq, strum::AsRefStr, Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum Expectation {
    Pass,
    Fail,
}

impl std::ops::Not for Expectation {
    type Output = Self;
    fn not(self) -> Self::Output {
        match self {
            Self::Fail => Self::Pass,
            Self::Pass => Self::Fail,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EvaluationConfig {
    #[serde(with = "humantime_serde")]
    pub stat_refresh_interval: Duration,
    pub paralegal_home_dir: PathBuf,
    pub app_config: HashMap<String, ApplicationConfig>,
    pub experiment: IndexMap<String, Box<[ExperimentConfig]>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ExperimentConfig {
    #[serde(flatten)]
    pub mode: ExperimentMode,
    #[serde(default = "const_true")]
    pub adaptive_depth: bool,
    #[serde(flatten)]
    pub application: Application,
    #[serde(default)]
    pub cargo_args: Box<[String]>,
    /// Default to the application name
    pub app_config_override: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ApplicationConfig {
    pub source_dir: PathBuf,
    #[serde(default)]
    pub cargo_args: Box<[String]>,
    #[serde(default = "const_true")]
    pub abort: bool,
    #[serde(default)]
    pub flow_args: Box<[String]>,
    pub external_annotations: Option<PathBuf>,
    /// Overwrites will be enacted in the same order that they are specified
    /// here.
    #[serde(default)]
    pub version_override: IndexMap<String, CrateOverride>,
}

fn const_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, strum::AsRefStr)]
#[serde(rename_all = "kebab-case", tag = "mode")]
#[strum(serialize_all = "kebab-case")]
pub enum ExperimentMode {
    #[serde(rename_all = "kebab-case")]
    RollForward {
        pass_threshold: Box<[String]>,
        fail_threshold: Box<[String]>,
        starting_expectation: Expectation,
        start_commit: String,
        limit: Option<usize>,
    },
    #[serde(rename_all = "kebab-case")]
    Ablation {
        feature_space_success: Box<[String]>,
        feature_space_fail: Box<[String]>,
    },
    CaseStudy,
}

#[derive(Serialize, Deserialize, strum::AsRefStr)]
#[serde(tag = "application", rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum Application {
    Plume,
    Lemmy {
        #[serde(default)]
        policies: Box<[lemmy::Prop]>,
    },
    Hyperswitch {
        #[serde(default)]
        policies: Box<[hyperswitch::Policy]>,
    },
    Websubmit {
        #[serde(default)]
        policies: Box<[websubmit::Policy]>,
    },
    AtomicData,
    Freedit {
        #[serde(default)]
        policies: Box<[freedit::Policy]>,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CrateOverride {
    pub replacement: Version,
    // The order in which these are overridden is not guaranteed
    pub original: VersionReq,
}
