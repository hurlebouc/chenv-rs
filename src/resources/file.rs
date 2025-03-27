use std::{
    fs,
    io::{self, BufReader, Write},
    path::Path,
};

use anyhow::{Context, Result, anyhow, bail};
use file_format::FileFormat;
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha256::try_digest;
use tempfile::tempdir;
use url::Url;
use zip::ZipArchive;

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
    url: InterpolableString,
    name: String,
    sha256: String,
    proxy: Option<String>,
    #[serde(default = "default_archive")]
    archive: bool,
    #[serde(default = "default_executable")]
    executable: bool,
}

fn default_archive() -> bool {
    false
}
fn default_executable() -> bool {
    false
}

#[cfg(target_family = "unix")]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut perm = std::fs::metadata(path)?.permissions();
    let mut mode = perm.mode();
    mode = mode + 0o111;
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
            fs::create_dir(&dest)?;
            let format = FileFormat::from_file(path)?;
            let file_reader = BufReader::new(std::fs::File::open(path)?);
            match format {
                FileFormat::Zip => {
                    if let Err(e) = ZipArchive::new(file_reader)?.extract(&dest) {
                        std::fs::remove_dir_all(&dest).with_context(|| {
                            format!("Cannot clean after error in extracting {:?}", dest)
                        })?;
                        bail!("Error extracting archive: {}", e);
                    }
                }
                FileFormat::TapeArchive => {
                    let mut archive = tar::Archive::new(file_reader);
                    archive
                        .unpack(&dest)
                        .with_context(|| format!("Cannot unpack tar {:?}", dest))?;
                }
                FileFormat::Gzip => {
                    let mut archive = tar::Archive::new(GzDecoder::new(file_reader));
                    archive
                        .unpack(&dest)
                        .with_context(|| format!("Cannot unpack tar.gz {:?}", dest))?;
                }
                _ => bail!(
                    "Unsupported archive format {} ({})",
                    format.name(),
                    format.extension()
                ),
            }
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
        return Ok(());
    }

    pub fn ensure_resources(&self, env: &Env, repo_location: &Path) -> Result<Substrate> {
        let output_dir = repo_location.join(&self.sha256);
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
