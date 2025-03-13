use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use anyhow::{Result, anyhow};
use config::Config;
use serde::{Deserialize, Serialize};

use crate::{
    interpol::{Env, InterpolableString},
    resources::Resource,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct Environment {
    resources: Option<HashMap<String, Resource>>,
    env: Option<HashMap<String, InterpolableString>>,
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

    pub fn ensure_resources(&self) -> Result<Env> {
        let mut resources = Env::new();
        if let Some(r) = &self.resources {
            for (k, v) in order_dependences(r)? {
                resources.insert(k.to_string(), v.ensure_resources(&resources)?);
            }
        }
        Ok(resources)
    }
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
