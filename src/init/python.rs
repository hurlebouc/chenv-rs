use std::io::{self, Read};

use anyhow::{Context, Result};
use reqwest::redirect;
use sha2::{Digest, Sha256};

use crate::{
    Os,
    config::{Environment, PathEnv},
    interpol::InterpolableString,
    resources::{self, Resource},
};

pub(crate) fn python() -> Result<Environment> {
    let client_with_redirect = reqwest::blocking::Client::builder()
        .redirect(redirect::Policy::default())
        .build()?;

    // Récupérer les informations sur les versions de Python
    let python_versions_res = client_with_redirect
        .get("https://www.python.org/api/v2/downloads/release/")
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::USER_AGENT, "chenv")
        .send()?;

    let python_versions_str = python_versions_res
        .error_for_status()
        .context("Failed to fetch Python versions")?
        .text()?;

    let python_versions_json = serde_json::from_str::<serde_json::Value>(&python_versions_str)?;

    // Extraire la dernière version stable
    let python_latest = python_versions_json
        .as_array()
        .context("Expected an array of Python versions")?
        .iter()
        .filter(|version| {
            version
                .get("is_published")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        })
        .max_by_key(|version| version.get("release_date").and_then(|v| v.as_str()))
        .context("No stable version found")?;
    let python_latest_version = &python_latest
        .get("name")
        .and_then(|v| v.as_str())
        .context("Failed to find the latest stable Python version")?[7..];

    // Construire les URLs pour le binaire
    let python_url = match Os::get() {
        Os::Linux => format!(
            "https://www.python.org/ftp/python/{python_latest_version}/Python-{python_latest_version}.tar.xz"
        ),
        Os::Windows => format!(
            "https://www.python.org/ftp/python/{python_latest_version}/python-{python_latest_version}-embed-amd64.zip"
        ),
        Os::MacOS => todo!("MacOS support is not implemented yet"),
    };

    let mut content = client_with_redirect
        .get(&python_url)
        .send()?
        .error_for_status()?;
    let mut hasher = Sha256::new();
    io::copy(&mut content, &mut hasher)?;
    let digest = hasher.finalize();
    let sha256 = format!("{:x}", digest);

    Ok(Environment {
        resources: Some(
            vec![(
                "python".to_string(),
                Resource::File {
                    repo_location: None,
                    file: resources::file::File {
                        url: InterpolableString::new(python_url),
                        name: "python".to_string(),
                        sha256: Some(sha256),
                        sha512: None,
                        proxy: None,
                        archive: true,
                        executable: false,
                    },
                },
            )]
            .into_iter()
            .collect(),
        ),
        env: Some(
            vec![(
                "PYTHON_HOME".to_string(),
                InterpolableString::new(format!("${{python}}/python")),
            )]
            .into_iter()
            .collect(),
        ),
        path: PathEnv(vec![InterpolableString::new(format!(
            "${{python}}/python/bin"
        ))]),
    })
}
