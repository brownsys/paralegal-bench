//! Types describing data the runner ingests

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Serialize, Deserialize)]
pub struct Config {
    #[serde(with = "humantime_serde")]
    pub stat_refresh_interval: Duration,
    pub app_config: HashMap<String, ApplicationConfig>,
    pub experiments: Box<[ExperimentConfig]>,
}

#[derive(Serialize, Deserialize)]
pub struct ExperimentConfig {
    pub r#type: ExperimentType,
    pub application: Application,
}

#[derive(Serialize, Deserialize)]
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
        policies: Box<[lemmy::Prop]>,
    },
    Hyperswitch {
        policies: Box<[hyperswitch::Policy]>,
    },
    WebSubmit {
        policies: Box<[websubmit::Policy]>,
    },
    AtomicData,
    Freedit {
        policies: Box<[freedit::Policy]>,
    },
}
