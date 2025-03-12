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

fn next_gen<'a>(
    deps: &HashMap<&'a String, HashSet<&'a String>>,
    descendant: impl Fn(&'a String) -> Vec<&'a String>,
) -> HashMap<&'a String, HashSet<&'a String>> {
    deps.iter()
        .map(|(k, v)| {
            (
                *k,
                v.iter()
                    .map(|dep| {
                        let mut desc = descendant(dep);
                        desc.push(dep);
                        desc
                    })
                    .flatten()
                    .collect::<HashSet<_>>(),
            )
        })
        .collect()
}

fn order_dependencies_gen<'a>(
    values: Vec<&'a String>,
    descendant: impl Fn(&'a String) -> Vec<&'a String>,
) -> Vec<&'a String> {
    let mut deps: HashMap<&'a String, HashSet<&'a String>> =
        HashMap::from_iter(values.iter().map(|k| (*k, HashSet::new())));
    let mut deps_next = next_gen(&deps, &descendant);
    while deps_next != deps {
        deps = deps_next;
        deps_next = next_gen(&deps, &descendant);
    }
    let ancestors = values
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
    while values.iter().any(|k| result.iter().all(|name| name != k)) {
        let mut has_new_element = false;
        for k in values.iter() {
            if result.iter().all(|(name)| name != k) {
                if ancestors[k]
                    .iter()
                    .all(|a| result.iter().any(|name| name == a))
                {
                    has_new_element = true;
                    result.push(*k);
                }
            }
        }
        if !has_new_element {
            panic!("Circular dependences detected");
        }
    }
    return result;
}

fn order_dependences<'a>(
    resources: &'a HashMap<String, Resource>,
) -> Vec<(&'a String, &'a Resource)> {
    let keys = resources.keys().collect::<Vec<_>>();
    let ordered_keys = order_dependencies_gen(keys.clone(), |k| {
        resources[k]
            .get_dependances()
            .into_iter()
            .collect::<Vec<_>>()
    });
    ordered_keys
        .into_iter()
        .map(|k| {
            (
                k,
                resources.get(k).expect("All keys should be in resources"),
            )
        })
        .collect()
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
