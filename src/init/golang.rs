use anyhow::{Context, Result};
use jsonpath_rust::JsonPath;
use reqwest::redirect;

use crate::{
    Os,
    config::{Environment, PathEnv},
    interpol::InterpolableString,
    resources::{self, Resource},
};

pub(crate) fn go() -> Result<Environment> {
    let client_with_redirect = reqwest::blocking::Client::builder()
        .redirect(redirect::Policy::default())
        .build()?;

    // Récupérer les informations sur les versions de Go
    let go_versions_res = client_with_redirect
        .get("https://go.dev/dl/?mode=json")
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::USER_AGENT, "chenv")
        .send()?;

    let go_versions_str = go_versions_res
        .error_for_status()
        .context("Failed to fetch Go versions")?
        .text()?;

    let go_versions_json = serde_json::from_str::<serde_json::Value>(&go_versions_str)?;

    // Extraire la dernière version stable
    let go_latest = go_versions_json
        .as_array()
        .context("Expected an array of Go versions")?
        .iter()
        .find(|version| {
            version
                .get("stable")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        })
        .context("No stable version found")?;
    let go_latest_version = go_latest
        .get("version")
        .and_then(|v| v.as_str())
        .context("Failed to find the latest stable Go version")?;
    let go_latest_sha256_all = go_latest
        .query("$.files[?@.os == \"linux\" && @.arch == \"amd64\"].sha256")
        .context("Failed to find the sha256 of the latest stable Go version")?;
    let go_latest_sha256 = go_latest_sha256_all
        .first()
        .context("no sha256 found")?
        .as_str()
        .context("sha256 should be a string")?;

    let go_latest_trimmed = go_latest_version.trim_start_matches('v'); // Supprimer le préfixe 'v'

    // Construire les URLs pour le binaire et le checksum
    let go_url = match Os::get() {
        Os::Linux => format!("https://go.dev/dl/{go_latest_trimmed}.linux-amd64.tar.gz"),
        Os::MacOS => format!("https://go.dev/dl/{go_latest_trimmed}.darwin-amd64.tar.gz"),
        Os::Windows => format!("https://go.dev/dl/{go_latest_trimmed}.windows-amd64.zip"),
    };

    Ok(Environment {
        resources: Some(
            vec![(
                "go".to_string(),
                Resource::File {
                    repo_location: None,
                    file: resources::file::File {
                        url: InterpolableString::new(go_url),
                        name: "go".to_string(),
                        sha256: Some(go_latest_sha256.to_string()),
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
                "GOROOT".to_string(),
                InterpolableString::new(format!("${{go}}/go/go")),
            )]
            .into_iter()
            .collect(),
        ),
        path: PathEnv(vec![InterpolableString::new(format!("${{go}}/go/go/bin"))]),
    })
}
