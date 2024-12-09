//! Types describing data the runner ingests

use indexmap::IndexMap;
use lemmy::eval_driver::{GetUserVersion, LemmyPackage};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, time::Duration};
use tracing::level_filters::LevelFilter;

#[derive(Clone, Copy, PartialEq, Eq, strum::AsRefStr, Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum PolicyResult {
    Pass,
    Fail,
}

impl std::ops::Not for PolicyResult {
    type Output = Self;
    fn not(self) -> Self::Output {
        match self {
            Self::Fail => Self::Pass,
            Self::Pass => Self::Fail,
        }
    }
}

mod ser_level_filter {
    use std::str::FromStr;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use tracing::level_filters::LevelFilter;

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<LevelFilter>, D::Error> {
        let str = <Option<String>>::deserialize(deserializer)?;
        str.map(|s| LevelFilter::from_str(&s).map_err(|e| serde::de::Error::custom(e)))
            .transpose()
    }

    pub fn serialize<S: Serializer>(
        l: &Option<LevelFilter>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        l.as_ref().map(ToString::to_string).serialize(serializer)
    }
}

#[derive(Serialize, Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum DumpCodeOption {
    #[default]
    None,
    Analyzed,
    Seen,
    Both,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EvaluationConfig {
    #[serde(with = "humantime_serde", default = "default_stat_refresh_interval")]
    pub stat_refresh_interval: Duration,
    pub paralegal_home_dir: PathBuf,
    #[serde(with = "ser_level_filter", default)]
    pub log_level: Option<LevelFilter>,
    pub app_config: HashMap<String, ApplicationConfig>,
    pub experiment: IndexMap<String, Box<[ExperimentConfig]>>,
    #[serde(with = "humantime_serde", default)]
    pub pdg_timeout: Option<Duration>,
    #[serde(default)]
    pub dump_analyzed_code: DumpCodeOption,
}

fn default_stat_refresh_interval() -> Duration {
    Duration::from_millis(500)
}

#[derive(Default, Debug, serde::Serialize, serde::Deserialize, Copy, Clone, strum::EnumIs)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyMode {
    #[default]
    Separate,
    Unified,
    None,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ExperimentConfig {
    #[serde(flatten)]
    pub mode: ExperimentMode,
    #[serde(default)]
    pub policy_mode: PolicyMode,
    #[serde(default = "const_true")]
    pub adaptive_depth: bool,
    #[serde(default = "const_true")]
    pub pdg_caching: bool,
    #[serde(flatten)]
    pub application: Application,
    #[serde(default)]
    pub cargo_args: Box<[String]>,
    #[serde(default = "const_false")]
    pub clean: bool,
    /// Default to the application name
    pub app_config_override: Option<String>,
    #[serde(default)]
    pub controller_run_mode: ControllerRunMode,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ApplicationConfig {
    pub source_dir: PathBuf,
    /// A git repository to clone into the application directory if it does not
    /// exist yet.
    pub clone: Option<String>,
    #[serde(default)]
    pub cargo_args: Box<[String]>,
    #[serde(default = "const_true")]
    pub abort: bool,
    #[serde(default)]
    pub flow_args: Box<[String]>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    pub external_annotations: Option<PathBuf>,
    /// Overwrites will be enacted in the same order that they are specified
    /// here.
    #[serde(default)]
    pub version_override: IndexMap<String, CrateOverride>,
}

fn const_true() -> bool {
    true
}

fn const_false() -> bool {
    false
}

#[derive(Serialize, Deserialize, strum::AsRefStr)]
#[serde(rename_all = "kebab-case", tag = "mode")]
#[strum(serialize_all = "kebab-case")]
pub enum ExperimentMode {
    #[serde(rename_all = "kebab-case")]
    RollForward {
        cutoff: Box<[RollForwardCutoff]>,
    },
    #[serde(rename_all = "kebab-case")]
    Ablation {
        feature_space_success: Box<[String]>,
        feature_space_fail: Box<[String]>,
    },
    CaseStudy,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "mode")]
pub struct RollForwardCutoff {
    /// Make sure not to specify external annotations in the app-config for this
    /// experiment or both will be passed
    pub external_annotations: Option<PathBuf>,
    /// If no expectation is set, this range of commits is skipped.
    pub expectation: Option<PolicyResult>,
    pub commit: String,
}

fn const_application_flavour() -> websubmit::Flavour {
    websubmit::Flavour::Application
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy, strum::AsRefStr)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum ControllerRunMode {
    All,
    AllSeparate,
    Affected,
    AffectedMerged,
}

impl Default for ControllerRunMode {
    fn default() -> Self {
        Self::All
    }
}

#[derive(Serialize, Deserialize, strum::AsRefStr)]
#[serde(tag = "application", rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum Application {
    Plume,
    #[serde(rename_all = "kebab-case")]
    Lemmy {
        #[serde(default)]
        policies: Box<[lemmy::Prop]>,
        #[serde(default)]
        bugs: Box<[GetUserVersion]>,
        new_version: Option<LemmyPackage>,
    },
    Hyperswitch {
        #[serde(default)]
        policies: Box<[hyperswitch::Policy]>,
    },
    Websubmit {
        #[serde(default)]
        policies: Box<[websubmit::Policy]>,
        #[serde(default = "const_application_flavour")]
        flavour: websubmit::Flavour,
    },
    AtomicData,
    Freedit {
        #[serde(default)]
        policies: Box<[freedit::Policy]>,
    },
    Contile {
        #[serde(default)]
        policies: Box<[contile::Policy]>,
    },
    Mcaptcha {
        #[serde(default)]
        policies: Box<[mCaptcha::Policy]>,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CrateOverride {
    pub replacement: Version,
    // The order in which these are overridden is not guaranteed
    pub original: VersionReq,
}
