//! Domain XML matching the project's `domain_reference.xml`.
//!
//! This models guest configuration, not installation orchestration: downloading
//! media, creating volumes, and generating cloud-init data happen separately.
//! Optional fields are omitted; `Some(Empty {})` emits a presence-only element.
//! Attribute values remain editable strings. The constructors use the settings
//! from the reference; libvirt validates values when defining the domain.
//! Deserialization supports this subset only; unmodeled XML is not preserved.
//!
//! See `README.md` in this directory for a complete construction example.
//! Reference: <https://libvirt.org/formatdomain.html>.

// The binary does not consume the model yet; these types are its construction API.
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
