mod file;

use std::{collections::HashMap, fmt::Debug};

use serde::{Deserialize, Serialize};

use crate::interpol::InterpolableString;

#[derive(Serialize, Deserialize, Debug)]
pub enum Resource {
    Archive {
        url: InterpolableString,
        sha256: String,
    },
    File(file::File),
    Git {
        url: InterpolableString,
        commit: String,
    },
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Substrate(String);

impl Substrate {
    pub fn to_string(&self) -> String {
        self.0.clone()
    }

    pub fn new(s: String) -> Self {
        Self(s)
    }
}

impl Resource {
    pub fn ensure_resources(&self, partial_resources: &HashMap<String, Substrate>) -> Substrate {
        todo!()
    }
    pub fn get_dependances(&self) -> Vec<&str> {
        match self {
            Resource::Archive { url, sha256 } => todo!(),
            Resource::Git { url, commit } => todo!(),
            Resource::File(file) => file.get_dependances(),
        }
    }
}
