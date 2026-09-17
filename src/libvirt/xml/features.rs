use serde::{Deserialize, Serialize};

use super::common::Empty;

/// Optional guest features enabled by the presence of their XML elements.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "features")]
pub struct Features {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acpi: Option<Empty>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apic: Option<Empty>,
}

impl Features {
    pub fn is_empty(&self) -> bool {
        self.acpi.is_none() && self.apic.is_none()
    }
}
