use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

#[derive(Deserialize, Serialize)]
pub struct VmConfig {
    pub user: GuestUserConfig,
    pub networking: NetworkConfig,
    pub instance: InstanceConfig,
}

#[derive(Deserialize, Serialize)]
pub struct GuestUserConfig {
    pub name: String,
    pub key: String,
}

#[derive(Deserialize, Serialize)]
pub struct NetworkConfig {
    pub ip: String,
    pub dhcp: bool,
    pub mac: String,
    pub remote_port: u16,
    pub service_port: u16,
}

#[derive(Deserialize, Serialize)]
pub struct InstanceConfig {
    pub hostname: String,
    #[serde(rename = "disk-size")]
    pub disk_size: usize,
    pub memory: u64,
    pub vcpus: u32,
}

#[derive(FromRow, Serialize)]
pub struct Instance {
    pub(crate) id: String,
    pub owner_id: String,
    pub(crate) hostname: String,
    memory_mib: u32,
    vcpus: u32,
    mac_address: String,
    ipv4_address: String,
    remote_port: u16,
    service_port: u16,
}

#[derive(Serialize)]
pub struct InstanceView {
    #[serde(flatten)]
    pub instance: Instance,
    pub role: crate::auth::permissions::Role,
    pub permissions: crate::auth::permissions::Permissions,
}
