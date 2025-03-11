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

fn next<'a>(
    resources: &'a HashMap<String, Resource>,
    deps: &HashMap<&'a String, HashSet<String>>,
) -> HashMap<&'a String, HashSet<String>> {
    let mut next_deps = deps.clone();
    for (k, v) in resources {
        for dep in v.get_dependances() {
            match next_deps.get_mut(k) {
                Some(set) => {
                    set.insert(dep);
                }
                None => panic!("Resource {} not found", k),
            }
        }
    }
    next_deps
}

fn order_dependences<'a>(
    resources: &'a HashMap<String, Resource>,
) -> Vec<(&'a String, &'a Resource)> {
    let keys = resources.keys().collect::<Vec<_>>();
    let mut deps: HashMap<&'a String, HashSet<String>> =
        HashMap::from_iter(keys.iter().map(|k| (*k, HashSet::new())));
    let mut deps_next = next(resources, &deps);
    while deps_next != deps {
        deps = deps_next;
        deps_next = next(resources, &deps);
    }
    let ancestors = keys
        .iter()
        .map(|k| {
            (
                *k,
                deps.iter()
                    .filter(|(_, v)| v.contains(*k))
                    .map(|(k, _)| *k)
                    .collect::<HashSet<_>>(),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut result = Vec::new();
    while keys
        .iter()
        .any(|k| result.iter().all(|(name, _)| name != k))
    {
        // TODO : relire le code de la boucle
        for k in keys.iter() {
            if !result.iter().any(|(name, _)| name == k) {
                if ancestors[k]
                    .iter()
                    .all(|a| result.iter().any(|(name, _)| name == a))
                {
                    result.push((*k, resources.get(*k).unwrap()));
                }
            }
        }
    }
    // if deps.iter().any(|(name, v)| v.contains(*name)) {
    //     panic!("Circular dependences detected");
    // }
    return result;
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
