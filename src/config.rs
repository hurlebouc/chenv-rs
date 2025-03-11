use std::collections::HashMap;

use config::Config;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Resource {}

#[derive(Serialize, Deserialize, Debug)]
pub struct Conf {
    pub resources: HashMap<String, Resource>,
    pub env: HashMap<String, String>,
}

pub fn read_config() -> Conf {
    let settings = Config::builder()
        .add_source(config::File::with_name("chenv"))
        .build()
        .unwrap();

    let conf = settings.try_deserialize::<Conf>().unwrap();

    conf
}
