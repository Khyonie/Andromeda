use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Os {
    #[serde(rename = "type")]
    pub r#type: OsType,
    #[serde(rename = "boot", default, skip_serializing_if = "Vec::is_empty")]
    pub boot: Vec<Boot>,
}

impl Default for Os {
    fn default() -> Self {
        Self {
            r#type: OsType {
                arch: "x86_64".into(),
                machine: "q35".into(),
                value: "hvm".into(),
            },
            boot: vec![Boot { dev: "hd".into() }],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OsType {
    #[serde(rename = "@arch")]
    pub arch: String,
    #[serde(rename = "@machine")]
    pub machine: String,
    #[serde(rename = "$text")]
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Boot {
    #[serde(rename = "@dev")]
    pub dev: String,
}
