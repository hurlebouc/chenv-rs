use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Serialize, Deserialize, Debug)]
pub enum Resource {
    Archive { url: Url, sha256: String },
    File { url: Url, sha256: String },
    Git { url: Url, commit: String },
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
    pub fn get_dependances(&self) -> Vec<String> {
        todo!()
    }
}
