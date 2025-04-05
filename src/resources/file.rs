use std::{
    fs,
    io::{self, BufReader, Write},
    path::Path,
};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use tempfile::tempdir;
use url::Url;

use crate::interpol::{Env, InterpolableString};

use super::Substrate;

#[cfg(target_family = "unix")]
use nix::sys::statvfs::statvfs;
#[cfg(target_family = "windows")]
use std::os::windows::ffi::OsStrExt;
#[cfg(target_family = "windows")]
use winapi::um::fileapi::GetVolumePathNameW;

#[derive(Serialize, Deserialize, Debug)]
pub struct File {
    pub url: InterpolableString,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha512: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,
    #[serde(default = "default_false")]
    #[serde(skip_serializing_if = "field_is_false")]
    pub archive: bool,
    #[serde(default = "default_false")]
    #[serde(skip_serializing_if = "field_is_false")]
    pub executable: bool,
}

fn field_is_false(v: &bool) -> bool {
    !v
}

fn default_false() -> bool {
    false
}

#[cfg(target_family = "unix")]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut perm = std::fs::metadata(path)?.permissions();
    let mut mode = perm.mode();
    mode += 0o111;
    perm.set_mode(mode);
    std::fs::set_permissions(path, perm)?;
    Ok(())
}

#[cfg(target_family = "windows")]
fn set_executable(path: &Path) -> Result<()> {
    Ok(())
}

fn move_file(src: &Path, dest: &Path) -> Result<()> {
    let dest_parent = dest
        .parent()
        .with_context(|| format!("cannot find parent for {dest:?}"))?;
    if is_on_same_fs(src, dest_parent)
        .with_context(|| format!("Cannot determine if {src:?} and {dest:?} are on the same disk"))?
    {
        // Same partition, use rename
        fs::rename(src, dest)?;
    } else {
        // Different partitions, use copy and remove
        fs::copy(src, dest)?;
        fs::remove_file(src)?;
    }
    Ok(())
}

fn is_on_same_fs(src: &Path, dest: &Path) -> Result<bool> {
    #[cfg(target_family = "unix")]
    {
        // println!(
        //     "src: {:?}, dest: {:?}",
        //     statvfs(src)?.filesystem_id(),
        //     statvfs(dest)?.filesystem_id()
        // );
        let src_stat = statvfs(src)?;
        let dest_stat = statvfs(dest)?;
        Ok(src_stat.filesystem_id() == dest_stat.filesystem_id())
    }

    #[cfg(target_family = "windows")]
    {
        let src_volume = get_volume_path(src)?;
        let dest_volume = get_volume_path(dest)?;
        Ok(src_volume == dest_volume)
    }
}

#[cfg(target_family = "windows")]
fn get_volume_path(path: &Path) -> Result<String> {
    let mut volume_path = vec![0u16; winapi::shared::minwindef::MAX_PATH];
    let path_str: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        if GetVolumePathNameW(
            path_str.as_ptr(),
            volume_path.as_mut_ptr(),
            volume_path.len() as u32,
        ) == 0
        {
            return Err(io::Error::last_os_error())?;
        }
    }
    let volume_path_str = String::from_utf16_lossy(&volume_path);
    Ok(volume_path_str.trim_end_matches('\u{0}').to_string())
}

#[derive(Debug, Clone, Copy)]
enum Sha<'a> {
    Sha256(&'a String),
    Sha512(&'a String),
}

impl<'a> Sha<'a> {
    fn compare(&self, path: &Path) -> Result<bool> {
        match self {
            Sha::Sha256(s) => {
                let mut hasher = Sha256::new();
                let file = std::fs::File::open(path)?;
                let mut reader = BufReader::new(file);
                io::copy(&mut reader, &mut hasher)?;
                let digest = hasher.finalize();
                let digest = format!("{:x}", digest);
                Ok(&digest == *s)
            }
            Sha::Sha512(s) => {
                let mut hasher = Sha512::new();
                let file = std::fs::File::open(path)?;
                let mut reader = BufReader::new(file);
                io::copy(&mut reader, &mut hasher)?;
                let digest = hasher.finalize();
                let digest = format!("{:x}", digest);
                Ok(&digest == *s)
            }
        }
    }

    fn small(&self) -> &'a str {
        match self {
            Sha::Sha256(s) => &s[..16],
            Sha::Sha512(s) => &s[..16],
        }
    }
}

impl File {
    fn import_file(
        &self,
        path: &Path,
        repo_location: &Path,
        orig_url: String,
        copy: bool,
        sha: &Sha,
    ) -> Result<()> {
        let output_dir = repo_location.join(sha.small());
        if !path.exists() {
            bail!("File {:?} is missing", path)
        }
        let dest = output_dir.join(&self.name);
        if !sha.compare(path)? {
            bail!("URL {} must have hash equal to {:?}", orig_url, sha)
        }
        fs::create_dir_all(&output_dir)?;
        if self.archive {
            mkar::unarchive(path, dest)?;
        } else {
            if copy {
                std::fs::copy(path, &dest)
                    .with_context(|| format!("Cannot copy {:?} into {:?}", path, dest))?;
            } else {
                move_file(path, &dest)
                    .with_context(|| format!("Cannot move {:?} into {:?}", path, dest))?;
            }
            if self.executable {
                set_executable(&dest)
                    .with_context(|| format!("Cannot make {:?} executable", dest))?;
            }
        }
        Ok(())
    }

    pub fn ensure_resources(&self, env: &Env, repo_location: &Path) -> Result<Substrate> {
        let sha = match (&self.sha256, &self.sha512) {
            (None, None) => bail!("Need sha256 or sha512"),
            (None, Some(s)) => Sha::Sha512(s),
            (Some(s), None) => Sha::Sha256(s),
            (Some(_), Some(_)) => bail!("Cannot have both sha256 and sha512"),
        };
        let output_dir = repo_location.join(sha.small());
        let output_file = output_dir.join(&self.name);
        let substrate = Substrate::new(
            std::path::absolute(output_dir)?
                .to_string_lossy()
                .to_string(),
        );
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
            self.import_file(&path, repo_location, url_str, true, &sha)?;
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
            self.import_file(&file_path, repo_location, url_str, false, &sha)?;
            return Ok(substrate);
        }
        bail!("Unsupported scheme {}", url.scheme());
    }
    pub fn get_dependances(&self) -> Vec<&str> {
        self.url.get_variables()
    }
}
