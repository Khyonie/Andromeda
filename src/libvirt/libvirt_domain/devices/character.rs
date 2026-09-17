use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Console {
    #[serde(rename = "@type")]
    pub r#type: String,
    pub target: ConsoleTarget,
}

impl Console {
    pub fn serial() -> Self {
        Self {
            r#type: "pty".into(),
            target: ConsoleTarget {
                r#type: "serial".into(),
                port: 0,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsoleTarget {
    #[serde(rename = "@type")]
    pub r#type: String,
    #[serde(rename = "@port")]
    pub port: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Channel {
    #[serde(rename = "@type")]
    pub r#type: String,
    pub target: ChannelTarget,
}

impl Channel {
    /// Libvirt chooses the Unix socket path for this guest-agent channel.
    pub fn guest_agent() -> Self {
        Self {
            r#type: "unix".into(),
            target: ChannelTarget {
                r#type: "virtio".into(),
                name: "org.qemu.guest_agent.0".into(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelTarget {
    #[serde(rename = "@type")]
    pub r#type: String,
    #[serde(rename = "@name")]
    pub name: String,
}
