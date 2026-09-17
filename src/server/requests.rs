use serde::Deserialize;

/// The existing JSON body shared by instance lookup and lifecycle endpoints.
#[derive(Deserialize)]
pub(super) struct InstanceRequest {
    pub id: String,
}
