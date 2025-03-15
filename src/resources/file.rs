use std::{
    fs,
    io::{self, BufReader, Write},
    path::Path,
};

use anyhow::{Result, anyhow, bail};
use file_format::FileFormat;
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha256::try_digest;
use tempfile::tempdir;
use url::Url;
use zip::ZipArchive;

use crate::interpol::{Env, InterpolableString};

use super::Substrate;

#[derive(Serialize, Deserialize, Debug)]
pub struct File {
    url: InterpolableString,
    name: String,
    sha256: String,
    proxy: Option<String>,
    #[serde(default = "default_archive")]
    archive: bool,
}

fn default_archive() -> bool {
    false
}

impl File {
    fn import_file(
        &self,
        path: &Path,
        repo_location: &Path,
        orig_url: String,
        copy: bool,
    ) -> Result<()> {
        let output_dir = repo_location.join(&self.sha256);
        if !path.exists() {
            bail!("File {:?} is missing", path)
        }
        let dest = output_dir.join(&self.name);
        let digest = try_digest(&path)?;
        if digest != self.sha256 {
            bail!(
                "URL {} has sha256 {} while expected is {}",
                orig_url,
                digest,
                self.sha256
            )
        }
        fs::create_dir_all(&output_dir)?;
        if self.archive {
            let format = FileFormat::from_file(path)?;
            let file_reader = BufReader::new(std::fs::File::open(path)?);
            match format {
                FileFormat::Zip => {
                    if let Err(e) = ZipArchive::new(file_reader)?.extract(&dest) {
                        std::fs::remove_dir_all(&dest)?;
                        bail!("Error extracting archive: {}", e);
                    }
                }
                FileFormat::TapeArchive => {
                    let mut archive = tar::Archive::new(file_reader);
                    archive.unpack(&dest)?;
                }
                FileFormat::Gzip => {
                    let mut archive = tar::Archive::new(GzDecoder::new(file_reader));
                    archive.unpack(&dest)?;
                }
                _ => bail!(
                    "Unsupported archive format {} ({})",
                    format.name(),
                    format.extension()
                ),
            }
        } else {
            if copy {
                std::fs::copy(path, dest)?;
            } else {
                std::fs::rename(path, dest)?;
            }
        }
        return Ok(());
    }

    pub fn ensure_resources(&self, env: &Env, repo_location: &Path) -> Result<Substrate> {
        let output_dir = repo_location.join(&self.sha256);
        let output_file = output_dir.join(&self.name);
        let substrate = Substrate::new(output_dir.to_string_lossy().to_string());
        if output_file.exists() {
            return Ok(substrate);
        }
        let url_str = self.url.interpolate(env)?;
        let url = url_str.parse::<Url>()?;
        println!("Get: {}", url);
        if url.scheme() == "file" {
            let path = url
                .to_file_path()
                .map_err(|()| anyhow!("Url {} is not a file", url))?;
            self.import_file(&path, repo_location, url_str, true)?;
            return Ok(substrate);
        }
        if url.scheme() == "http" || url.scheme() == "https" {
            let tmpdir = tempdir()?;
            let file_path = tmpdir.path().join(&self.name);
            let client = match &self.proxy {
                Some(proxy) => reqwest::blocking::Client::builder()
                    .proxy(reqwest::Proxy::all(proxy)?)
                    .build()?,
                None => reqwest::blocking::Client::new(),
            };
            let body = client.get(url.clone()).send()?;
            let mut body_reader = BufReader::new(body);
            let mut file = std::fs::File::create_new(&file_path)?;
            io::copy(&mut body_reader, &mut file)?;
            file.flush()?;
            self.import_file(&file_path, repo_location, url_str, false)?;
            return Ok(substrate);
        }
        bail!("Unsupported scheme {}", url.scheme());
    }
    pub fn get_dependances(&self) -> Vec<&str> {
        self.url.get_variables()
    }
}
