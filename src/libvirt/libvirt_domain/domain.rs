use serde::{Deserialize, Serialize};

use crate::libvirt::libvirt_domain::Empty;

use super::{cpu::Cpu, devices::Devices, features::Features, memory::Memory, os::Os};

/// Domain configuration matching `domain_reference.xml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "domain")]
pub struct Domain {
    #[serde(rename = "@type")]
    pub r#type: String,
    pub name: String,
    /// Libvirt generates a UUID when this is omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    pub memory: Memory,
    pub vcpu: u32,
    pub os: Os,
    #[serde(default, skip_serializing_if = "Features::is_empty")]
    pub features: Features,
    pub cpu: Cpu,
    pub devices: Devices,
}

impl Domain {
    /// A KVM guest with the reference's x86_64/q35, hard-disk boot, and
    /// host-passthrough CPU settings. Attach devices before defining the guest.
    pub fn new(name: impl Into<String>, memory_mib: u64, vcpus: u32) -> Self {
        Self {
            r#type: "kvm".into(),
            name: name.into(),
            uuid: None,
            memory: Memory::mib(memory_mib),
            vcpu: vcpus,
            os: Os::default(),
            features: Features {
                acpi: Some(Empty {}),
                apic: Some(Empty {}),
            },
            cpu: Cpu {
                mode: "host-passthrough".into(),
            },
            devices: Devices::default(),
        }
    }

    /// Serialize for `virt::domain::Domain::define_xml` or `create_xml`.
    /// Libvirt validates values and hardware compatibility when accepting XML.
    pub fn to_xml(&self) -> Result<String, quick_xml::SeError> {
        quick_xml::se::to_string(self)
    }
}
