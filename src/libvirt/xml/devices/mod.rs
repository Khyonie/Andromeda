use serde::{Deserialize, Serialize};

pub mod character;
pub mod network;
pub mod storage;

pub use character::*;
pub use network::*;
pub use storage::*;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Devices {
    #[serde(rename = "disk", default, skip_serializing_if = "Vec::is_empty")]
    pub disks: Vec<Disk>,
    #[serde(rename = "interface", default, skip_serializing_if = "Vec::is_empty")]
    pub interfaces: Vec<Interface>,
    #[serde(rename = "console", default, skip_serializing_if = "Vec::is_empty")]
    pub consoles: Vec<Console>,
    #[serde(rename = "channel", default, skip_serializing_if = "Vec::is_empty")]
    pub channels: Vec<Channel>,
}
