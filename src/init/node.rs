use anyhow::{Context, Result};
use reqwest::redirect;

use crate::{
    Os,
    config::{Environment, PathEnv},
    interpol::InterpolableString,
    resources::{self, Resource},
};

pub(crate) fn node() -> Result<Environment> {
    let client_with_redirect = reqwest::blocking::Client::builder()
        .redirect(redirect::Policy::default())
        .build()?;

    // Récupérer les informations sur les versions de Node.js
    let node_versions_res = client_with_redirect
        .get("https://nodejs.org/dist/index.json")
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::USER_AGENT, "chenv")
        .send()?;

    let node_versions_str = node_versions_res
        .error_for_status()
        .context("Failed to fetch Node.js versions")?
        .text()?;

    let node_versions_json = serde_json::from_str::<serde_json::Value>(&node_versions_str)?;

    // Extraire la dernière version LTS
    let node_latest = node_versions_json
        .as_array()
        .context("Expected an array of Node.js versions")?
        .iter()
        .find(|version| {
            version
                .get("lts")
                .and_then(|v| v.as_bool().or_else(|| v.as_str().map(|_| true)))
                .unwrap_or(false)
        })
        .context("No LTS version found")?;
    let node_latest_version = node_latest
        .get("version")
        .and_then(|v| v.as_str())
        .context("Failed to find the latest LTS Node.js version")?;

    let os_str = match Os::get() {
        Os::Linux => "linux",
        Os::MacOS => "darwin",
        Os::Windows => "win",
    };

    let archive_fmt = match Os::get() {
        Os::Linux => "tar.xz",
        Os::MacOS => "tar.xz",
        Os::Windows => "zip",
    };

    // Construire les URLs pour le binaire et le checksum
    let node_sha256_url = format!("https://nodejs.org/dist/{node_latest_version}/SHASUMS256.txt");
    let node_url = format!(
        "https://nodejs.org/dist/{node_latest_version}/node-{node_latest_version}-{os_str}-x64.{archive_fmt}"
    );

    // Récupérer le checksum SHA256
    let node_sha256_res = client_with_redirect
        .get(node_sha256_url)
        .send()?
        .error_for_status()?;

    let node_sha256_text = node_sha256_res.text()?;
    // println!("{}", node_sha256_text);
    let node_sha256 = node_sha256_text
        .lines()
        .find(|line| {
            line.contains(&format!(
                "node-{node_latest_version}-{os_str}-x64.{archive_fmt}"
            ))
        })
        .and_then(|line| line.split_whitespace().next())
        .context("Failed to find the SHA256 checksum for the latest Node.js version")?;

    Ok(Environment {
        resources: Some(
            vec![(
                "node".to_string(),
                Resource::File {
                    repo_location: None,
                    file: resources::file::File {
                        url: InterpolableString::new(node_url),
                        name: "node".to_string(),
                        sha256: Some(node_sha256.to_string()),
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
                "NODE_HOME".to_string(),
                InterpolableString::new(format!(
                    "${{node}}/node/node-{node_latest_version}-{os_str}-x64"
                )),
            )]
            .into_iter()
            .collect(),
        ),
        path: PathEnv(vec![InterpolableString::new(format!(
            "${{node}}/node/node-{node_latest_version}-{os_str}-x64/bin"
        ))]),
    })
}
