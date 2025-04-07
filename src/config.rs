use std::{
    collections::{HashMap, HashSet},
    path::Path,
    str::from_utf8,
};

use anyhow::{Context, Result, anyhow, bail};
use jsonpath_rust::JsonPath;
use reqwest::redirect;
use serde::{Deserialize, Serialize};

use crate::{
    Os,
    interpol::{Env, InterpolableString},
    resources::{self, Resource, Substrate},
};

#[derive(Serialize, Deserialize, Debug)]
struct Host {
    env: HashMap<String, String>,
}

impl Host {
    fn new() -> Self {
        Self {
            env: std::env::vars_os()
                .map(|(k, v)| {
                    (
                        k.to_string_lossy().to_string(),
                        v.to_string_lossy().to_string(),
                    )
                })
                .collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PathEnv(pub Vec<InterpolableString>);

impl PathEnv {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Environment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<HashMap<String, Resource>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, InterpolableString>>,
    #[serde(skip_serializing_if = "PathEnv::is_empty", default)]
    pub path: PathEnv,
}

impl Environment {
    pub fn get_env<'a>(&'a self, env: &Env) -> Result<HashMap<&'a String, String>> {
        let mut result = HashMap::new();
        if let Some(e) = &self.env {
            for (k, v) in e {
                result.insert(k, env.interpolate(v)?);
            }
        }
        Ok(result)
    }

    pub fn get_path<'c>(&'c self, env: &Env) -> Result<Vec<String>> {
        let mut result = Vec::new();
        for v in &self.path.0 {
            result.push(env.interpolate(v)?);
        }
        Ok(result)
    }

    pub fn ensure_resources(&self, config_parent: &Path) -> Result<Env> {
        let mut resources = Env::new();
        resources.insert("host".to_string(), Substrate::new(Host::new()));
        if let Some(r) = &self.resources {
            for (k, v) in order_dependences(r)? {
                resources.insert(
                    k.to_string(),
                    v.ensure_resources(&resources, config_parent)?,
                );
            }
        }
        Ok(resources)
    }

    pub(crate) fn merge(self, other: Environment) -> Result<Self> {
        let env = match (self.env, other.env) {
            (None, None) => None,
            (None, Some(env)) => Some(env),
            (Some(env), None) => Some(env),
            (Some(env1), Some(env2)) => Some(merge_maps(env1, env2)?),
        };
        let resources = match (self.resources, other.resources) {
            (None, None) => None,
            (None, Some(resources)) => Some(resources),
            (Some(resources), None) => Some(resources),
            (Some(r1), Some(r2)) => Some(merge_maps(r1, r2)?),
        };
        let path = PathEnv(self.path.0.into_iter().chain(other.path.0).collect());
        Ok(Self {
            env,
            resources,
            path,
        })
    }
}

fn merge_maps<V>(map1: HashMap<String, V>, map2: HashMap<String, V>) -> Result<HashMap<String, V>>
where
    V: Eq,
{
    let mut merged = map1;
    for (k, v) in map2.into_iter() {
        if let Some(false) = merged.get(&k).map(|previous| previous == &v) {
            bail!("Both maps have same key ({k}) with different values");
        }
        merged.insert(k, v);
    }
    Ok(merged)
}

fn next_gen<'a>(
    deps: &HashMap<&'a str, HashSet<&'a str>>,
    descendant: impl Fn(&'a str) -> Vec<&'a str>,
) -> HashMap<&'a str, HashSet<&'a str>> {
    deps.iter()
        .map(|(k, v)| {
            (
                *k,
                v.iter()
                    .flat_map(|dep| {
                        let mut desc = descendant(dep);
                        desc.push(dep);
                        desc
                    })
                    .collect::<HashSet<_>>(),
            )
        })
        .collect()
}

fn order_dependencies_gen<'a>(
    values: Vec<&'a str>,
    descendant: impl Fn(&'a str) -> Vec<&'a str>,
) -> Result<Vec<&'a str>> {
    let mut deps: HashMap<&'a str, HashSet<&'a str>> = HashMap::from_iter(
        values
            .iter()
            .map(|k| (*k, HashSet::from_iter(descendant(k)))),
    );
    let mut deps_next = next_gen(&deps, &descendant);
    while deps_next != deps {
        deps = deps_next;
        deps_next = next_gen(&deps, &descendant);
    }
    let mut result = Vec::new();
    // tant qu'il reste des éléments à ajouter
    while values.iter().any(|k| result.iter().all(|name| name != k)) {
        let mut has_new_element = false;
        for k in values.iter() {
            // si k n'est pas déjà dans result...
            if result.iter().all(|name| name != k) {
                // ...et si tous les dépendances de k sont dans result
                if deps[k].iter().all(|a| result.iter().any(|name| name == a)) {
                    has_new_element = true;
                    result.push(*k);
                }
            }
        }
        // si on n'a pas ajouté d'élément alors qu'il reste des éléments à ajouter,
        // c'est qu'il y a une dépendance circulaire
        if !has_new_element {
            return Err(anyhow!("Circular dependences detected"));
        }
    }
    return Ok(result);
}

fn order_dependences<'a>(
    resources: &'a HashMap<String, Resource>,
) -> Result<Vec<(&'a str, &'a Resource)>> {
    let keys = resources.keys().map(|k| k.as_str()).collect::<Vec<_>>();
    let ordered_keys = order_dependencies_gen(keys.clone(), |k| resources[k].get_dependances())?;
    Ok(ordered_keys
        .into_iter()
        .map(|k| {
            (
                k,
                resources.get(k).expect("All keys should be in resources"),
            )
        })
        .collect())
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BuildEnvironment {
    pub cmd: String,
    #[serde(flatten)]
    pub env: Environment,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct Conf {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<Environment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub builder: Option<BuildEnvironment>,
}

pub fn read_config(path: &Path) -> Result<Conf> {
    let file = std::fs::File::open(path)?;
    Ok(serde_yaml::from_reader(file)?)
}

pub fn read_config_in_repo(path: &Path) -> Result<Conf> {
    let file = std::fs::File::open(path.join("chenv.yaml"))?;
    Ok(serde_yaml::from_reader(file)?)
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    #[test]
    fn test_order_dependencies_same_length_path() -> Result<()> {
        use std::collections::HashMap;

        use super::order_dependencies_gen;

        let a = "a";
        let b = "b";
        let c = "c";
        let d = "d";
        let e = "e";

        let values = vec![a, b, c, d, e];
        let deps = vec![
            (e, vec![b, c]),
            (b, vec![d]),
            (c, vec![d]),
            (d, vec![a]),
            (a, vec![]),
        ]
        .into_iter()
        .collect::<HashMap<_, _>>();

        let result = order_dependencies_gen(values, |k| {
            deps.get(k)
                .expect("All keys should be in deps")
                .iter()
                .map(|v| *v)
                .collect()
        })?;

        assert!(result == vec!["a", "d", "c", "b", "e"] || result == vec!["a", "d", "b", "c", "e"]);
        Ok(())
    }

    #[test]
    fn test_order_dependencies_diff_length_path() -> Result<()> {
        use std::collections::HashMap;

        use super::order_dependencies_gen;

        let a = "a";
        let b = "b";
        let c = "c";
        let d = "d";
        let e = "e";
        let f = "f";

        let values = vec![f, e, d, c, b, a];
        let deps = vec![
            (e, vec![f]),
            (b, vec![e]),
            (f, vec![]),
            (a, vec![b, c]),
            (d, vec![e]),
            (c, vec![d]),
        ]
        .into_iter()
        .collect::<HashMap<_, _>>();

        let result = order_dependencies_gen(values, |k| {
            deps.get(k)
                .expect("All keys should be in deps")
                .iter()
                .map(|v| *v)
                .collect()
        })?;

        assert!(
            result == vec!["f", "e", "d", "c", "b", "a"]
                || result == vec!["f", "e", "d", "b", "c", "a"]
        );
        Ok(())
    }

    #[test]
    fn test_order_dependencies_2_roots() -> Result<()> {
        use std::collections::HashMap;

        use super::order_dependencies_gen;

        let a = "a";
        let b = "b";
        let c = "c";
        let d = "d";
        let e = "e";

        let values = vec![e, c, d, b, a];
        let deps = vec![
            (e, vec![]),
            (b, vec![d]),
            (a, vec![c]),
            (d, vec![c]),
            (c, vec![e]),
        ]
        .into_iter()
        .collect::<HashMap<_, _>>();

        let result = order_dependencies_gen(values, |k| {
            deps.get(k)
                .expect("All keys should be in deps")
                .iter()
                .map(|v| *v)
                .collect()
        })?;

        assert!(result == vec!["e", "c", "d", "b", "a"] || result == vec!["e", "c", "d", "a", "b"]);
        Ok(())
    }

    #[test]
    fn test_order_dependencies_cycle() -> Result<()> {
        use std::collections::HashMap;

        use super::order_dependencies_gen;

        let a = "a";
        let b = "b";
        let c = "c";
        let d = "d";
        let e = "e";

        let values = vec![e, c, d, b, a];
        let deps = vec![
            (e, vec![d]),
            (b, vec![d]),
            (a, vec![c]),
            (d, vec![c]),
            (c, vec![e]),
        ]
        .into_iter()
        .collect::<HashMap<_, _>>();

        let result = order_dependencies_gen(values, |k| {
            deps.get(k)
                .expect("All keys should be in deps")
                .iter()
                .map(|v| *v)
                .collect()
        });

        assert!(result.is_err());

        Ok(())
    }
}
