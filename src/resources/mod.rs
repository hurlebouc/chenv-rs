use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Serialize, Deserialize, Debug)]
pub enum Resource {
    Archive { url: Url, sha256: String },
    File { url: Url, sha256: String },
    Git { url: Url, commit: String },
}
