//! Instance workflows shared by the HTTP layer and future application entry points.

use sqlx::SqlitePool;
use virt::connect::Connect;

use crate::{
    cloud_init,
    config::Flags,
    database::instances as database,
    libvirt,
    logging::{OperationLog, Severity, SharedLogger},
    model::{Instance, VmConfig},
    paths::StoragePaths,
    storage,
};

#[derive(Debug)]
pub enum ErrorKind {
    InvalidInput,
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
    Created,
    DryRun,
}

#[derive(Clone)]
pub struct InstanceService {
    pub(crate) database: SqlitePool,
    pub(crate) logger: SharedLogger,
    qemu: Connect,
    flags: Flags,
    paths: StoragePaths,
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
    id: &str,
) -> Result<Instance, InstanceError> {
    let mut operation = OperationLog::new(logger, "get", Some(id));
    let instance = lookup_instance(database, id, &mut operation).await?;
    operation.info("Completed: returning instance data");
    Ok(instance)
}

pub async fn get_instance_ids(
    database: &SqlitePool,
    logger: SharedLogger,
) -> Result<Vec<String>, InstanceError> {
    let mut operation = OperationLog::new(logger, "list", None);
    operation.step("Reading instance IDs from database");
    let ids = database::get_instance_ids(database)
        .await
        .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
    operation.info(format!("Completed: returning {} instance IDs", ids.len()));
    Ok(ids)
}

impl InstanceService {
    pub fn new(
        database: SqlitePool,
        qemu: Connect,
        flags: Flags,
        paths: StoragePaths,
        logger: SharedLogger,
    ) -> Self {
        Self {
            database,
            qemu,
            flags,
            paths,
            logger,
        }
    }

    pub async fn create(&self, config: VmConfig) -> Result<CreateOutcome, InstanceError> {
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
        operation.step("Validating instance paths");
        let paths = self
            .paths
            .instance(&config.instance.hostname)
            .map_err(|e| operation_error(&operation, ErrorKind::InvalidInput, e))?;
        operation.step("Generating cloud-init data");
        let user_data = cloud_init::user_seed(&config);
        let network_config = cloud_init::network_config_seed(&config);
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
        database::insert_instance(&self.database, &id, &config)
            .await
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;

        operation.info(format!("Completed: registered new domain with UUID {id}"));
        Ok(CreateOutcome::Created)
    }

    pub async fn delete(&self, id: &str) -> Result<(), InstanceError> {
        let mut operation = OperationLog::new(self.logger.clone(), "delete", Some(id));
        let instance = lookup_instance(&self.database, id, &mut operation).await?;
        operation.step("Looking up libvirt domain");
        let domain = libvirt::lookup_domain(&self.qemu, &instance.id)
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;

        operation.step("Checking whether domain is active");
        if libvirt::is_active(&domain)
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?
        {
            return Err(operation_error(
                &operation,
                ErrorKind::Conflict,
                "Domain is currently active. Destroy it before attempting undefine.",
            ));
        }

        operation.step("Undefining libvirt domain");
        libvirt::undefine_domain(&domain)
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;

        operation.step("Deleting instance from database");
        database::delete_instance_by_id(&self.database, id)
            .await
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;

        operation.info("Completed: instance deleted");
        Ok(())
    }

    pub async fn start(&self, id: &str) -> Result<(), InstanceError> {
        let mut operation = OperationLog::new(self.logger.clone(), "start", Some(id));
        let instance = lookup_instance(&self.database, id, &mut operation).await?;
        operation.step("Looking up libvirt domain");
        let domain = libvirt::lookup_domain(&self.qemu, &instance.id)
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
        operation.step("Starting libvirt domain");
        libvirt::start_domain(&domain)
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
        operation.info("Completed: instance started");
        Ok(())
    }

    pub async fn stop(&self, id: &str) -> Result<(), InstanceError> {
        let mut operation = OperationLog::new(self.logger.clone(), "stop", Some(id));
        let instance = lookup_instance(&self.database, id, &mut operation).await?;
        operation.step("Looking up libvirt domain");
        let domain = libvirt::lookup_domain(&self.qemu, &instance.id)
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
        operation.step("Stopping libvirt domain");
        libvirt::stop_domain(&domain)
            .map_err(|e| operation_error(&operation, ErrorKind::Internal, e))?;
        operation.info("Completed: instance stopped");
        Ok(())
    }
}
