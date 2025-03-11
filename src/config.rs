use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use config::Config;
use serde::{Deserialize, Serialize};

use crate::{interpol::Env, resources::Resource};

#[derive(Serialize, Deserialize, Debug)]
pub struct Environment {
    resources: Option<HashMap<String, Resource>>,
    env: Option<HashMap<String, String>>,
}

impl Environment {
    pub fn get_env<'a>(&'a self, env: &Env) -> HashMap<&'a String, String> {
        let mut result = HashMap::new();
        if let Some(e) = &self.env {
            for (k, v) in e {
                result.insert(k, env.interpolate(v));
            }
        }
        result
    }

    pub fn ensure_resources(&self) -> Env {
        let mut resources = HashMap::new();
        if let Some(r) = &self.resources {
            for (k, v) in order_dependences(r) {
                resources.insert(k.clone(), v.ensure_resources(&resources));
            }
        }
        Env(resources)
    }
}

fn order_dependences<'a>(
    resources: &'a HashMap<String, Resource>,
) -> Vec<(&'a String, &'a Resource)> {
    let mut deps: HashMap<&'a String, HashSet<&'a String>> = HashMap::new();
    todo!()
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

pub fn read_config(path: &Option<PathBuf>) -> Conf {
    let file = match path {
        Some(p) => config::File::from(p.as_path()),
        None => config::File::with_name("chenv"),
    };
    let settings = Config::builder().add_source(file).build().unwrap();

    let conf = settings.try_deserialize::<Conf>().unwrap();

    conf
}
