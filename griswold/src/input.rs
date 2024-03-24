//! Types describing data the runner ingests

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

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
    pub mode: ExperimentMode,
    #[serde(default = "const_true")]
    pub adaptive_depth: bool,
    #[serde(flatten)]
    pub application: Application,
    #[serde(default)]
    pub cargo_args: Box<[String]>,
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
}

fn const_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, strum::AsRefStr)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum ExperimentMode {
    RollForward,
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
    WebSubmit {
        #[serde(default)]
        policies: Box<[websubmit::Policy]>,
    },
    AtomicData,
    Freedit {
        #[serde(default)]
        policies: Box<[freedit::Policy]>,
    },
}
