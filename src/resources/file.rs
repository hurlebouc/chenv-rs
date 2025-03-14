use std::{
    collections::HashMap,
    fs,
    io::{self, BufReader, Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use sha256::try_digest;
use tempfile::{tempdir, tempfile};
use url::Url;

use crate::interpol::{Env, InterpolableString};

use super::Substrate;

#[derive(Serialize, Deserialize, Debug)]
pub struct File {
    url: InterpolableString,
    name: String,
    sha256: String,
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
        fs::create_dir_all(output_dir)?;
        if copy {
            std::fs::copy(path, dest)?;
        } else {
            std::fs::rename(path, dest)?;
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
            //println!("{:?}", file_path);
            let body = reqwest::blocking::get(url.clone())?;
            let mut body_reader = BufReader::new(body);

            let mut file = std::fs::File::create_new(&file_path)?;
            io::copy(&mut body_reader, &mut file)?;
            file.flush()?;
            self.import_file(&file_path, repo_location, url_str, false)?;
            return Ok(substrate);
        }
        todo!()
    }
    pub fn get_dependances(&self) -> Vec<&str> {
        self.url.get_variables()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() -> Result<()> {
        let issue_list_url = Url::parse("/a/b/c")?;
        print!("{}", issue_list_url.scheme());
        Ok(())
    }
}
