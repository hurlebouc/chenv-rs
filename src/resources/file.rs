use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::interpol::{Env, InterpolableString};

use super::Substrate;

#[derive(Serialize, Deserialize, Debug)]
pub struct File {
    url: InterpolableString,
    sha256: String,
}

impl File {
    pub fn ensure_resources(&self, env: &Env, repo_location: &Path) -> Result<Substrate> {
        let url_str = self.url.interpolate(env)?;
        let url = url_str.parse::<Url>()?;
        if url.scheme() == "file" {
            todo!()
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
