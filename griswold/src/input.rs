//! Types describing data the runner ingests

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    #[serde(with = "humantime_serde")]
    pub stat_refresh_interval: Duration,
    pub paralegal_home_dir: PathBuf,
    pub app_config: HashMap<String, ApplicationConfig>,
    pub experiments: Box<[ExperimentConfig]>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ExperimentConfig {
    pub r#type: ExperimentType,
    #[serde(flatten)]
    pub application: Application,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ApplicationConfig {
    pub source_dir: PathBuf,
}

#[derive(Serialize, Deserialize, strum::AsRefStr)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum ExperimentType {
    RollForward,
    Ablation,
    CaseStudy,
    AdaptiveInlining,
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
