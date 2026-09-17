//! Domain XML matching `tests/fixtures/domain.xml`.
//!
//! This models guest configuration, not installation orchestration: downloading
//! media, creating volumes, and generating cloud-init data happen separately.
//! Optional fields are omitted; `Some(Empty {})` emits a presence-only element.
//! Attribute values remain editable strings. The constructors use the settings
//! from the reference; libvirt validates values when defining the domain.
//! Deserialization supports this subset only; unmodeled XML is not preserved.
//!
//! See the root `README.md` for the optional libvirt schema validation command.
//! Reference: <https://libvirt.org/formatdomain.html>.

// Public XML types provide a construction API beyond the fields used by provisioning.
#![allow(dead_code, unused_imports)]

pub mod common;
pub mod cpu;
pub mod devices;
pub mod domain;
pub mod features;
pub mod memory;
pub mod os;

pub use common::*;
pub use cpu::*;
pub use devices::*;
pub use domain::*;
pub use features::*;
pub use memory::*;
pub use os::*;

#[cfg(test)]
mod tests;
