use serde::Serialize;
use sqlx::SqlitePool;

use super::Account;
use crate::instances::{ErrorKind, InstanceError};

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Owner,
    Operator,
    Viewer,
}

#[derive(Clone, Copy)]
pub enum Permission {
    View,
    Control,
    Download,
    Delete,
}

#[derive(Serialize)]
pub struct Permissions {
    pub view: bool,
    pub control: bool,
    pub download: bool,
    pub delete: bool,
    pub manage_access: bool,
}

impl Role {
    pub fn permissions(self) -> Permissions {
        Permissions {
            view: true,
            control: matches!(self, Self::Owner | Self::Operator),
            download: matches!(self, Self::Owner),
            delete: matches!(self, Self::Owner),
            manage_access: matches!(self, Self::Owner),
        }
    }
}

pub async fn require(
    database: &SqlitePool,
    account: &Account,
    instance_id: &str,
    permission: Permission,
) -> Result<Role, InstanceError> {
    let role: Option<String> = sqlx::query_scalar(
        "SELECT CASE WHEN i.owner_id = ? THEN 'owner' ELSE m.role END \
         FROM instances i LEFT JOIN instance_members m ON m.instance_id = i.id AND m.user_id = ? \
         WHERE i.id = ? AND (i.owner_id = ? OR m.user_id IS NOT NULL)",
    )
    .bind(&account.id)
    .bind(&account.id)
    .bind(instance_id)
    .bind(&account.id)
    .fetch_optional(database)
    .await
    .map_err(|_| InstanceError {
        kind: ErrorKind::Internal,
        message: "Could not check instance access".into(),
    })?;
    let role = match role.as_deref() {
        Some("owner") => Role::Owner,
        Some("operator") => Role::Operator,
        Some("viewer") => Role::Viewer,
        _ => {
            return Err(InstanceError {
                kind: ErrorKind::NotFound,
                message: "No such instance".into(),
            });
        }
    };
    let permissions = role.permissions();
    let allowed = match permission {
        Permission::View => permissions.view,
        Permission::Control => permissions.control,
        Permission::Download => permissions.download,
        Permission::Delete => permissions.delete,
    };
    if !allowed {
        return Err(InstanceError {
            kind: ErrorKind::Forbidden,
            message: "Your role does not permit this operation".into(),
        });
    }
    Ok(role)
}
