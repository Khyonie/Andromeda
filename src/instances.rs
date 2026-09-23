//! Instance workflows shared by the HTTP layer and future application entry points.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};

use crate::auth::{
    Account,
    permissions::{self, Permission},
};
use sqlx::SqlitePool;
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};
use virt::connect::Connect;
use virt::error::ErrorNumber;

use crate::{
    cloud_init,
    config::Flags,
    database::instances as database,
    libvirt,
    logging::{OperationLog, Severity, SharedLogger},
    model::{Instance, InstanceListEntry, InstanceView, VmConfig},
    paths::StoragePaths,
    settings::SettingsService,
    storage,
};

#[derive(Debug)]
pub enum ErrorKind {
    InvalidInput,
    Forbidden,
    NotFound,
    Conflict,
    Internal,
}

#[derive(Debug)]
pub struct InstanceError {
    pub kind: ErrorKind,
    pub message: String,
}

impl std::fmt::Display for InstanceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for InstanceError {}

pub enum CreateOutcome {
    Created { id: String },
    DryRun,
}

#[derive(Clone)]
pub struct InstanceService {
    pub(crate) database: SqlitePool,
    pub(crate) logger: SharedLogger,
    qemu: Connect,
    flags: Flags,
    paths: StoragePaths,
    settings: SettingsService,
    locks: Arc<Mutex<HashMap<String, Weak<AsyncMutex<()>>>>>,
    metrics: Arc<libvirt::metrics::Metrics>,
}

fn operation_error(
    operation: &OperationLog,
    kind: ErrorKind,
    error: impl std::fmt::Display,
) -> InstanceError {
    let severity = if matches!(kind, ErrorKind::Internal) {
        Severity::Error
    } else {
        Severity::Warning
    };
    operation.failure(severity, &error);
    InstanceError {
        kind,
        message: error.to_string(),
    }
}

async fn lookup_instance(
    database: &SqlitePool,
    id: &str,
    operation: &mut OperationLog,
) -> Result<Instance, InstanceError> {
    operation.step("Looking up instance in database");
    database::get_instance_by_id(database, id)
        .await
        .map_err(|e| operation_error(operation, ErrorKind::Internal, e))?
        .ok_or_else(|| operation_error(operation, ErrorKind::NotFound, "No such instance"))
}

pub async fn get_instance(
    database: &SqlitePool,
    logger: SharedLogger,
    account: &Account,
    id: &str,
) -> Result<InstanceView, InstanceError> {
    let mut operation = OperationLog::new(logger, "get", Some(id));
    let instance = lookup_instance(database, id, &mut operation).await?;
    operation.step("Checking instance access");
    let role = permissions::require(database, account, id, Permission::View)
        .await
        .map_err(|e| operation_error(&operation, e.kind, e.message))?;
    operation.info("Completed: returning instance data");
    Ok(InstanceView {
        instance,
        role,
        permissions: role.permissions(),
    })
}

pub async fn get_instance_ids(
    database: &SqlitePool,
    logger: SharedLogger,
    account: &Account,
) -> Result<Vec<String>, InstanceError> {
    let mut operation = OperationLog::new(logger, "list", None);
    operation.step("Reading instance IDs from database");
    let ids = database::get_instance_ids(database, &account.id)
        .await
        .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
    operation.info(format!("Completed: returning {} instance IDs", ids.len()));
    Ok(ids)
}

impl InstanceService {
    pub async fn list(&self, account: &Account) -> Result<Vec<InstanceListEntry>, InstanceError> {
        let mut operation = OperationLog::new(self.logger.clone(), "list", None);
        operation.step("Reading instance summaries from database");
        let instances = database::get_instance_summaries(&self.database, &account.id)
            .await
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
        operation.step("Reading instance power states");
        let qemu = self.qemu.clone();
        let task_operation = operation.clone();
        let entries = tokio::task::spawn_blocking(move || {
            instances
                .into_iter()
                .map(|instance| {
                    let state = match libvirt::power_state(&qemu, &instance.id) {
                        Ok(state) => state,
                        Err(error) => {
                            task_operation.message(
                                Severity::Warning,
                                format!("Could not read power state for {}: {error}", instance.id),
                            );
                            "unavailable"
                        }
                    }
                    .into();
                    InstanceListEntry { instance, state }
                })
                .collect::<Vec<_>>()
        })
        .await
        .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
        operation.info(format!(
            "Completed: returning {} instance summaries",
            entries.len()
        ));
        Ok(entries)
    }

    pub fn new(
        database: SqlitePool,
        qemu: Connect,
        flags: Flags,
        paths: StoragePaths,
        settings: SettingsService,
        logger: SharedLogger,
    ) -> Self {
        Self {
            database,
            qemu,
            flags,
            paths,
            settings,
            logger,
            locks: Arc::default(),
            metrics: Arc::default(),
        }
    }

    fn lock(
        &self,
        hostname: &str,
        operation: &OperationLog,
    ) -> Result<OwnedMutexGuard<()>, InstanceError> {
        let lock = {
            let mut locks = self.locks.lock().unwrap();
            locks.retain(|_, lock| lock.strong_count() > 0);
            let lock = locks
                .get(hostname)
                .and_then(Weak::upgrade)
                .unwrap_or_default();
            locks.insert(hostname.into(), Arc::downgrade(&lock));
            lock
        };
        lock.try_lock_owned().map_err(|_| operation_error(operation, ErrorKind::Conflict,
            "Another operation is in progress for this instance. Please try again when it finishes."))
    }

    async fn locked_instance(
        &self,
        id: &str,
        account: &Account,
        permission: Permission,
        operation: &mut OperationLog,
    ) -> Result<(Instance, OwnedMutexGuard<()>), InstanceError> {
        let instance = lookup_instance(&self.database, id, operation).await?;
        let guard = self.lock(&instance.hostname, operation)?;
        // Recheck after acquiring the lock: a concurrent delete may have finished meanwhile.
        let instance = lookup_instance(&self.database, id, operation).await?;
        operation.step("Checking instance access");
        permissions::require(&self.database, account, id, permission)
            .await
            .map_err(|e| operation_error(operation, e.kind, e.message))?;
        Ok((instance, guard))
    }

    pub async fn status(
        &self,
        account: &Account,
        id: &str,
    ) -> Result<libvirt::metrics::InstanceStatus, InstanceError> {
        let mut operation = OperationLog::new(self.logger.clone(), "status", Some(id));
        let instance = lookup_instance(&self.database, id, &mut operation).await?;
        permissions::require(&self.database, account, id, Permission::View).await?;
        let qemu = self.qemu.clone();
        let metrics = self.metrics.clone();
        operation.step("Reading live instance statistics");
        let status = tokio::task::spawn_blocking(move || metrics.read(&qemu, &instance.id))
            .await
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
        operation.info("Completed: returning live instance statistics");
        Ok(status)
    }

    pub async fn download_disk(
        &self,
        account: &Account,
        id: &str,
    ) -> Result<storage::DiskExport, InstanceError> {
        let mut operation = OperationLog::new(self.logger.clone(), "download disk", Some(id));
        let (instance, guard) = self
            .locked_instance(id, account, Permission::Download, &mut operation)
            .await?;
        operation.step("Checking whether domain is inactive");
        let domain = libvirt::lookup_domain(&self.qemu, &instance.id)
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
        if libvirt::is_active(&domain)
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?
        {
            return Err(operation_error(
                &operation,
                ErrorKind::Conflict,
                "Shut down the instance before downloading its disk.",
            ));
        }
        let paths = self
            .paths
            .instance(&instance.hostname)
            .map_err(|e| operation_error(&operation, ErrorKind::InvalidInput, e))?;
        let export_root = self.paths.data_directory().join("exports");
        // Keep the operation lock inside the blocking task even if the HTTP client disconnects.
        let task_operation = operation.clone();
        tokio::task::spawn_blocking(move || {
            let _guard = guard;
            let mut operation = task_operation;
            storage::export_disk(&paths, &export_root, &mut operation)
                .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))
        })
        .await
        .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?
    }

    pub async fn create(
        &self,
        account: &Account,
        mut config: VmConfig,
    ) -> Result<CreateOutcome, InstanceError> {
        let mut operation = OperationLog::new(
            self.logger.clone(),
            "create",
            Some(&config.instance.hostname),
        );
        if self.flags.dry_run {
            operation.info(
                "Dry run: disk commands, domain definition, and database insertion will be skipped",
            );
        }
        operation.step("Validating instance description");
        config.instance.description = config.instance.description.trim().to_owned();
        if config.instance.description.chars().count() > 2000
            || config.instance.description.contains('\0')
        {
            return Err(operation_error(
                &operation,
                ErrorKind::InvalidInput,
                "Description must be at most 2000 characters and cannot contain NUL characters.",
            ));
        }
        operation.step("Validating instance paths");
        let paths = self
            .paths
            .instance(&config.instance.hostname)
            .map_err(|e| operation_error(&operation, ErrorKind::InvalidInput, e))?;
        let _guard = self.lock(&config.instance.hostname, &operation)?;
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM instances WHERE hostname = ?)")
                .bind(&config.instance.hostname)
                .fetch_one(&self.database)
                .await
                .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
        if exists
            || paths.disk.symlink_metadata().is_ok()
            || paths.seed_iso.symlink_metadata().is_ok()
        {
            return Err(operation_error(
                &operation,
                ErrorKind::Conflict,
                "This hostname or its storage already exists. Choose another hostname.",
            ));
        }
        operation.step("Reading guest defaults");
        let defaults = self
            .settings
            .snapshot()
            .await
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
        if config.user.name == "sysadmin" {
            return Err(operation_error(
                &operation,
                ErrorKind::InvalidInput,
                "The username sysadmin is reserved for the configured system administrator.",
            ));
        }
        operation.step("Generating cloud-init data");
        let user_data = cloud_init::user_seed(&config, &defaults.sysadmin_ssh_key);
        let network_config = cloud_init::network_config_seed(&config, defaults.gateway_ip);
        let meta_data = cloud_init::metadata_seed(&config);

        operation.step("Writing temporary seed files");
        let seeds =
            cloud_init::SeedFiles::write(&user_data, &network_config, &meta_data, &operation)
                .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;

        operation.step("Preparing instance disk");
        storage::create_image(&paths, config.instance.disk_size, &self.flags, &operation)
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;

        operation.step("Preparing cloud-init ISO");
        storage::create_iso(&paths, &seeds, &self.flags, &operation)
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
        drop(seeds);

        operation.step("Generating domain XML");
        let (xml, id) = libvirt::generate_domain(&config, &paths)
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;

        if self.flags.dry_run {
            operation.info(format!(
                "Completed dry run: would define and register domain with UUID {id}"
            ));
            return Ok(CreateOutcome::DryRun);
        }

        operation.step("Defining libvirt domain");
        libvirt::define_domain(&self.qemu, &xml)
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
        operation.info(format!("Defined domain with UUID {id}"));

        operation.step("Inserting instance into database");
        database::insert_instance(&self.database, &id, &config, &account.id)
            .await
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;

        operation.info(format!("Completed: registered new domain with UUID {id}"));
        Ok(CreateOutcome::Created { id: id.to_string() })
    }

    pub async fn delete(&self, account: &Account, id: &str) -> Result<(), InstanceError> {
        let mut operation = OperationLog::new(self.logger.clone(), "delete", Some(id));
        let (instance, _guard) = self
            .locked_instance(id, account, Permission::Delete, &mut operation)
            .await?;
        let paths = self
            .paths
            .instance(&instance.hostname)
            .map_err(|e| operation_error(&operation, ErrorKind::InvalidInput, e))?;
        operation.step("Validating instance storage");
        storage::check_instance_files(&paths)
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
        operation.step("Looking up libvirt domain");
        match libvirt::lookup_domain(&self.qemu, &instance.id) {
            Ok(domain) => {
                operation.step("Checking whether domain is active");
                if libvirt::is_active(&domain)
                    .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?
                {
                    return Err(operation_error(
                        &operation,
                        ErrorKind::Conflict,
                        "Shut down the instance before deleting it.",
                    ));
                }
                operation.step("Undefining libvirt domain");
                libvirt::undefine_domain(&domain)
                    .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
            }
            // Retry cleanup after a partial failure, retaining the SQL record until everything is removed.
            Err(error) if error.code() == ErrorNumber::NoDomain => {
                operation.info("Domain already absent; continuing storage cleanup")
            }
            Err(error) => return Err(operation_error(&operation, ErrorKind::Internal, error)),
        }
        storage::delete_instance_files(&paths, &mut operation)
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
        operation.step("Deleting instance from database");
        database::delete_instance_by_id(&self.database, id)
            .await
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
        self.metrics.forget(id);
        operation.info("Completed: instance, disk, and seed ISO deleted");
        Ok(())
    }

    pub async fn start(&self, account: &Account, id: &str) -> Result<(), InstanceError> {
        let mut operation = OperationLog::new(self.logger.clone(), "start", Some(id));
        let (instance, _guard) = self
            .locked_instance(id, account, Permission::Control, &mut operation)
            .await?;
        operation.step("Looking up libvirt domain");
        let domain = libvirt::lookup_domain(&self.qemu, &instance.id)
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
        self.metrics.forget(id);
        operation.step("Starting libvirt domain");
        libvirt::start_domain(&domain)
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
        operation.info("Completed: instance started");
        Ok(())
    }

    pub async fn stop(&self, account: &Account, id: &str) -> Result<(), InstanceError> {
        let mut operation = OperationLog::new(self.logger.clone(), "stop", Some(id));
        let (instance, _guard) = self
            .locked_instance(id, account, Permission::Control, &mut operation)
            .await?;
        operation.step("Looking up libvirt domain");
        let domain = libvirt::lookup_domain(&self.qemu, &instance.id)
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
        operation.step("Stopping libvirt domain");
        libvirt::stop_domain(&domain)
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
        operation.info("Completed: instance stopped");
        Ok(())
    }

    pub async fn destroy(&self, account: &Account, id: &str) -> Result<(), InstanceError> {
        let mut operation = OperationLog::new(self.logger.clone(), "stop", Some(id));
        let (instance, _guard) = self
            .locked_instance(id, account, Permission::Control, &mut operation)
            .await?;
        operation.step("Looking up libvirt domain");
        let domain = libvirt::lookup_domain(&self.qemu, &instance.id)
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
        operation.step("Stopping libvirt domain");
        libvirt::destroy_domain(&domain)
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
        operation.info("Completed: instance stopped");
        Ok(())
    }
}
