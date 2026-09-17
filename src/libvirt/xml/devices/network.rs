use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Interface {
    #[serde(rename = "@type")]
    pub r#type: String,
    /// Libvirt generates a MAC address when this is omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac: Option<Mac>,
    pub source: InterfaceSource,
    pub model: InterfaceModel,
}

impl Interface {
    pub fn network(name: impl Into<String>) -> Self {
        Self {
            r#type: "network".into(),
            mac: None,
            source: InterfaceSource {
                network: name.into(),
            },
            model: InterfaceModel {
                r#type: "virtio".into(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mac {
    #[serde(rename = "@address")]
    pub address: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceSource {
    #[serde(rename = "@network")]
    pub network: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceModel {
    #[serde(rename = "@type")]
    pub r#type: String,
}
