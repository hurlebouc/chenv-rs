use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::interpol::InterpolableString;

use super::Substrate;

#[derive(Serialize, Deserialize, Debug)]
pub struct File {
    url: InterpolableString,
    sha256: String,
}

impl File {
    pub fn ensure_resources(&self, partial_resources: &HashMap<String, Substrate>) -> Substrate {
        todo!()
    }
    pub fn get_dependances(&self) -> Vec<&str> {
        self.url.get_variables()
    }
}
