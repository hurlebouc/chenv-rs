use std::collections::HashMap;

use config::Config;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Serialize, Deserialize, Debug)]
pub enum Resource {
    Archive { url: Url, sha256: String },
    File { url: Url, sha256: String },
    Git { url: Url, commit: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Environment {
    pub resources: Option<HashMap<String, Resource>>,
    pub env: Option<HashMap<String, String>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BuildEnvironment {
    pub cmd: String,
    #[serde(flatten)]
    pub env: Environment,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Conf {
    pub shell: Option<Environment>,
    pub builder: Option<BuildEnvironment>,
}

pub fn read_config() -> Conf {
    let settings = Config::builder()
        .add_source(config::File::with_name("chenv"))
        .build()
        .unwrap();

    let conf = settings.try_deserialize::<Conf>().unwrap();

    conf
}
