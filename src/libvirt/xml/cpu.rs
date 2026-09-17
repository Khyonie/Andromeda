use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cpu {
    #[serde(rename = "@mode")]
    pub mode: String,
}
