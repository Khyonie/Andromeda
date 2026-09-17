use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
    #[serde(rename = "@unit")]
    pub unit: String,
    #[serde(rename = "$text")]
    pub value: u64,
}

impl Memory {
    pub fn mib(value: u64) -> Self {
        Self {
            unit: "MiB".into(),
            value,
        }
    }
}
