use std::str::from_utf8;

use anyhow::{Context, Ok, Result, bail};
use jsonpath_rust::JsonPath;
use reqwest::redirect;

use crate::{
    Os,
    config::{Environment, PathEnv},
    interpol::InterpolableString,
    resources::{self, Resource},
};

pub(crate) fn java(version: u8) -> Result<Environment> {
    let client = reqwest::blocking::Client::builder()
        .redirect(redirect::Policy::none())
        .build()?;
    let client_with_redirect = reqwest::blocking::Client::builder()
        .redirect(redirect::Policy::default())
        .build()?;
    let version_response = client.get(format!("https://api.adoptium.net/v3/info/release_names?image_type=jdk&jvm_impl=hotspot&release_type=ga&semver=false&version=[{}.0,{}.0)", version, version+1)).send()?.error_for_status()?;
    let version_json = serde_json::from_str::<serde_json::Value>(&version_response.text()?)?;
    let release_name = if let serde_json::Value::Object(map) = version_json {
        if let Some(serde_json::Value::Array(list)) = map.get("releases") {
            if let Some(serde_json::Value::String(release_name)) = list.first() {
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
    let (java_location_url, java_sha_url) = match Os::get() {
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
    Ok(Environment {
        resources: Some(
            vec![(
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
            )]
            .into_iter()
            .collect(),
        ),
        env: Some(
            vec![(
                "JAVA_HOME".to_string(),
                InterpolableString::new(format!("${{java}}/jdk/{release_name}")),
            )]
            .into_iter()
            .collect(),
        ),
        path: PathEnv(vec![InterpolableString::new(format!(
            "${{java}}/jdk/{release_name}/bin"
        ))]),
    })
}

pub(crate) fn maven() -> Result<Environment> {
    let client_with_redirect = reqwest::blocking::Client::builder()
        .redirect(redirect::Policy::default())
        .build()?;
    let mvn_latest_req = client_with_redirect
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
    let mvn_sha512= client_with_redirect.get(format!("https://repo1.maven.org/maven2/org/apache/maven/apache-maven/{mvn_latest}/apache-maven-{mvn_latest}-bin.zip.sha512")).send()?.error_for_status()?.text()?;
    let mvn_url = format!(
        "https://repo1.maven.org/maven2/org/apache/maven/apache-maven/{mvn_latest}/apache-maven-{mvn_latest}-bin.zip"
    );
    Ok(Environment {
        resources: Some(
            vec![(
                "maven".to_string(),
                Resource::File {
                    repo_location: None,
                    file: resources::file::File {
                        url: InterpolableString::new(mvn_url),
                        name: "mvn".to_string(),
                        sha256: None,
                        sha512: Some(mvn_sha512),
                        proxy: None,
                        archive: true,
                        executable: false,
                    },
                },
            )]
            .into_iter()
            .collect(),
        ),
        env: None,
        path: PathEnv(vec![InterpolableString::new(format!(
            "${{maven}}/mvn/apache-maven-{mvn_latest}/bin"
        ))]),
    })
}

pub(crate) fn sbt() -> Result<Environment> {
    let client_with_redirect = reqwest::blocking::Client::builder()
        .redirect(redirect::Policy::default())
        .build()?;
    let sbt_latest_req = client_with_redirect
                    .get("https://search.maven.org/solrsearch/select?q=g:org.scala-sbt+AND+a:sbt-launch+AND+v:1.*&wt=json")
                    .header(reqwest::header::ACCEPT, "application/json")
                    .header(reqwest::header::USER_AGENT, "chenv");
    let sbt_latest_res = sbt_latest_req.send()?;
    //println!("{mvn_latest_res:?}");
    let sbt_latest_str = sbt_latest_res
        .error_for_status()
        .context("Failed to get maven-core version")?
        .text()?;
    //println!("{mvn_latest_str}");
    let sbt_latest_json = serde_json::from_str::<serde_json::Value>(&sbt_latest_str)?;
    let sbt_latest = match sbt_latest_json.query("$.response.docs[0].v")?[0] {
        serde_json::Value::String(s) => s,
        _ => bail!("cannot find latest version"),
    };
    // println!("{mvn_latest}");
    let sbt_sha256 = client_with_redirect
        .get(format!(
            "https://github.com/sbt/sbt/releases/download/v{sbt_latest}/sbt-{sbt_latest}.zip.sha256"
        ))
        .send()?
        .error_for_status()?
        .text()?
        .split(" ")
        .next()
        .context("sha256 response must be of the forme <SHA256> <RELEASE_NAME>")?
        .to_string();
    let sbt_url =
        format!("https://github.com/sbt/sbt/releases/download/v{sbt_latest}/sbt-{sbt_latest}.zip",);
    Ok(Environment {
        resources: Some(
            vec![(
                "sbt".to_string(),
                Resource::File {
                    repo_location: None,
                    file: resources::file::File {
                        url: InterpolableString::new(sbt_url),
                        name: "sbt".to_string(),
                        sha256: Some(sbt_sha256),
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
        env: None,
        path: PathEnv(vec![InterpolableString::new(format!(
            "${{sbt}}/sbt/sbt/bin"
        ))]),
    })
}
