mod file;

use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::interpol::{Env, InterpolableString};

#[derive(Serialize, Deserialize, Debug)]
pub enum Resource {
    Archive {
        url: InterpolableString,
        sha256: String,
    },
    File {
        repo_location: Option<PathBuf>,
        #[serde(flatten)]
        file: file::File,
    },
    Git {
        url: InterpolableString,
        commit: String,
    },
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Substrate(serde_json::Value);

impl Substrate {
    pub fn to_string(&self) -> String {
        match &self.0 {
            serde_json::Value::String(a) => a.clone(),
            _ => self.0.to_string(),
        }
    }

    pub fn new<T: Serialize>(t: T) -> Self {
        Self(serde_json::to_value(t).unwrap())
    }
}

impl Resource {
    pub fn ensure_resources(&self, env: &Env) -> Result<Substrate> {
        match self {
            Resource::Archive { url, sha256 } => todo!(),
            Resource::Git { url, commit } => todo!(),
            Resource::File {
                repo_location,
                file,
            } => file.ensure_resources(
                env,
                repo_location.clone().unwrap_or("./.chenv".into()).as_path(),
            ),
        }
    }
    pub fn get_dependances(&self) -> Vec<&str> {
        match self {
            Resource::Archive { url, sha256 } => todo!(),
            Resource::Git { url, commit } => todo!(),
            Resource::File {
                repo_location: _,
                file,
            } => file.get_dependances(),
        }
    }
}
