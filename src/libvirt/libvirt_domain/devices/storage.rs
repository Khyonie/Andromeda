use serde::{Deserialize, Serialize};

use super::super::common::Empty;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Disk {
    #[serde(rename = "@type")]
    pub r#type: String,
    #[serde(rename = "@device")]
    pub device: String,
    pub driver: DiskDriver,
    pub source: DiskSource,
    pub target: DiskTarget,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readonly: Option<Empty>,
}

impl Disk {
    /// Attach an existing image as a virtio disk; storage is created separately.
    pub fn file(
        path: impl Into<String>,
        target: impl Into<String>,
        format: impl Into<String>,
    ) -> Self {
        Self {
            r#type: "file".into(),
            device: "disk".into(),
            driver: DiskDriver {
                name: "qemu".into(),
                r#type: format.into(),
            },
            source: DiskSource { file: path.into() },
            target: DiskTarget {
                dev: target.into(),
                bus: "virtio".into(),
            },
            readonly: None,
        }
    }

    /// Attach an ISO as a read-only SATA CD-ROM.
    pub fn cdrom(path: impl Into<String>, target: impl Into<String>) -> Self {
        let mut disk = Self::file(path, target, "raw");
        disk.device = "cdrom".into();
        disk.target.bus = "sata".into();
        disk.readonly = Some(Empty {});
        disk
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskDriver {
    #[serde(rename = "@name")]
    pub name: String,
    /// Image format, such as `qcow2` or `raw`.
    #[serde(rename = "@type")]
    pub r#type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskSource {
    #[serde(rename = "@file")]
    pub file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskTarget {
    #[serde(rename = "@dev")]
    pub dev: String,
    #[serde(rename = "@bus")]
    pub bus: String,
}
