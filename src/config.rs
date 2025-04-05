use std::{
    collections::{HashMap, HashSet},
    os,
    path::{Path, PathBuf},
    str::from_utf8,
};

use anyhow::{Context, Result, anyhow, bail};
use config::{Config, Source};
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

#[derive(Serialize, Deserialize, Debug)]
pub struct Environment {
    #[serde(skip_serializing_if = "Option::is_none")]
    resources: Option<HashMap<String, Resource>>,
    #[serde(skip_serializing_if = "Option::is_none")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<Environment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub builder: Option<BuildEnvironment>,
}

pub fn read_config(path: &Path) -> Result<Conf> {
    let file = config::File::from(path);
    let settings = Config::builder().add_source(file).build()?;
    let conf = settings.try_deserialize::<Conf>()?;
    Ok(conf)
}

pub fn read_config_in_repo(path: &Path) -> Result<Conf> {
    let file = config::File::with_name(&path.join("chenv").to_string_lossy().to_string());
    let settings = Config::builder().add_source(file).build()?;
    let conf = settings.try_deserialize::<Conf>()?;
    Ok(conf)
}

impl Conf {
    pub(crate) fn init_java(os: &Os) -> Result<Conf> {
        let client = reqwest::blocking::Client::builder()
            .redirect(redirect::Policy::none())
            .build()?;
        let client_with_redirect = reqwest::blocking::Client::builder()
            .redirect(redirect::Policy::default())
            .build()?;
        let version_response = client.get("https://api.adoptium.net/v3/info/release_names?image_type=jdk&jvm_impl=hotspot&release_type=ga&semver=false&version=[8.0,9.0)").send()?.error_for_status()?;
        let version_json = serde_json::from_str::<serde_json::Value>(&version_response.text()?)?;
        let release_name = if let serde_json::Value::Object(map) = version_json {
            if let Some(serde_json::Value::Array(list)) = map.get("releases") {
                if let Some(serde_json::Value::String(release_name)) = list.get(0) {
                    release_name.to_owned()
                } else {
                    bail!(
                        "version json must be en object with filed \"releases\" which is a non empty array of strings"
                    )
                }
            } else {
                bail!("version json must be en object with filed \"releases\" which is an array")
            }
        } else {
            bail!("version json must be an object")
        };
        let (java_location_url, java_sha_url) = match os {
            Os::Linux => (
                format!(
                    "https://api.adoptium.net/v3/binary/version/{release_name}/linux/x64/jdk/hotspot/normal/eclipse"
                ),
                format!(
                    "https://api.adoptium.net/v3/checksum/version/{release_name}/linux/x64/jdk/hotspot/normal/eclipse"
                ),
            ),
            Os::MacOS => (
                format!(
                    "https://api.adoptium.net/v3/binary/version/{release_name}/mac/x64/jdk/hotspot/normal/eclipse"
                ),
                format!(
                    "https://api.adoptium.net/v3/checksum/version/{release_name}/mac/x64/jdk/hotspot/normal/eclipse"
                ),
            ),
            Os::Windows => (
                format!(
                    "https://api.adoptium.net/v3/binary/version/{release_name}/windows/x64/jdk/hotspot/normal/eclipse"
                ),
                format!(
                    "https://api.adoptium.net/v3/checksum/version/{release_name}/windows/x64/jdk/hotspot/normal/eclipse"
                ),
            ),
        };
        let location_response = client.get(java_location_url).send()?.error_for_status()?;
        let sha256_response = client_with_redirect
            .get(java_sha_url)
            .send()?
            .error_for_status()?;
        // println!("{sha256_response:?}");
        let sha256_bytes = sha256_response.bytes()?;
        let sha256_file = from_utf8(&sha256_bytes)?;
        // println!("{sha256_file}");
        let java_sha256 = sha256_file
            .split(" ")
            .next()
            .context("sha256 response must be of the forme <SHA256> <RELEASE_NAME>")?;
        let java_url = match location_response.headers().get("location") {
            Some(location) => location.to_str()?,
            None => bail!("Response must redirect to binary"),
        };

        let mvn_latest_req = client
            .get("https://search.maven.org/solrsearch/select?q=g:org.apache.maven+AND+a:maven-core&wt=json")
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::USER_AGENT, "chenv");
        //println!("{mvn_latest_req:?}");
        let mvn_latest_res = mvn_latest_req.send()?;
        //println!("{mvn_latest_res:?}");
        let mvn_latest_str = mvn_latest_res
            .error_for_status()
            .context("Failed to get maven-core version")?
            .text()?;
        //println!("{mvn_latest_str}");
        let mvn_latest_json = serde_json::from_str::<serde_json::Value>(&mvn_latest_str)?;
        let mvn_latest = match mvn_latest_json.query("$.response.docs[0].latestVersion")?[0] {
            serde_json::Value::String(s) => s,
            _ => bail!("cannot find latest version"),
        };
        // println!("{mvn_latest}");

        let mvn_url = format!(
            "https://dlcdn.apache.org/maven/maven-4/{mvn_latest}/binaries/apache-maven-{mvn_latest}-bin.zip"
        );
        let mvn_sha512= client_with_redirect.get(format!("https://downloads.apache.org/maven/maven-4/{mvn_latest}/binaries/apache-maven-{mvn_latest}-bin.zip.sha512")).send()?.error_for_status()?.text()?;

        return Ok(Conf {
            shell: Some(Environment {
                resources: Some(
                    vec![
                        (
                            "java".to_string(),
                            Resource::File {
                                repo_location: None,
                                file: resources::file::File {
                                    url: InterpolableString::new(java_url.to_string()),
                                    name: "jdk".to_string(),
                                    sha256: Some(java_sha256.to_string()),
                                    sha512: None,
                                    proxy: None,
                                    archive: true,
                                    executable: false,
                                },
                            },
                        ),
                        (
                            "maven".to_string(),
                            Resource::File {
                                repo_location: None,
                                file: resources::file::File {
                                    url: InterpolableString::new(mvn_url.to_string()),
                                    name: "mvn".to_string(),
                                    sha256: None,
                                    sha512: Some(mvn_sha512.to_string()),
                                    proxy: None,
                                    archive: true,
                                    executable: false,
                                },
                            },
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
                env: Some(
                    vec![
                        (
                            match os {
                                Os::Linux => "PATH".to_string(),
                                Os::Windows => "Path".to_string(),
                                Os::MacOS => "PATH".to_string(),
                            },
                            InterpolableString::new(match os {
                                Os::Linux => format!(
                                    "${{java}}/jdk/{release_name}/bin:${{maven}}/mvn/apache-maven-{mvn_latest}/bin:${{host.env.PATH}}"
                                ),
                                Os::MacOS => format!(
                                    "${{java}}/jdk/{release_name}/bin:${{maven}}/mvn/apache-maven-{mvn_latest}/bin:${{host.env.PATH}}"
                                ),
                                Os::Windows => format!(
                                    "${{java}}\\jdk\\{release_name}\\bin;${{maven}}\\mvn\\apache-maven-{mvn_latest}\\bin;${{host.env.Path}}"
                                ),
                            }),
                        ),
                        (
                            "JAVA_HOME".to_string(),
                            InterpolableString::new(format!("${{java}}/jdk/{release_name}")),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
            }),
            builder: None,
        });
    }
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
