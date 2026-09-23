use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::{
    cloud_init::SeedFiles,
    config::Flags,
    logging::OperationLog,
    paths::{InstancePaths, skeleton_directory},
};

pub(crate) fn create_image(
    paths: &InstancePaths,
    size_mib: usize,
    flags: &Flags,
    operation: &OperationLog,
) -> Result<()> {
    let skeleton = skeleton_directory();
    anyhow::ensure!(
        skeleton.is_dir(),
        "Guest skeleton directory is missing: {}",
        skeleton.display()
    );
    run(
        Command::new("qemu-img")
            .args(["create", "-f", "qcow2", "-F", "qcow2", "-b"])
            .arg(&paths.cloud_image)
            .arg(&paths.disk),
        flags,
        operation,
    )?;
    run(
        Command::new("qemu-img")
            .arg("resize")
            .arg(&paths.disk)
            .arg(format!("{size_mib}M")),
        flags,
        operation,
    )?;
    operation.info("Copying guest defaults into /etc/skel");
    // copy-in places the directory itself in the destination, including dotfiles.
    let mut copy_source = skeleton.into_os_string();
    copy_source.push(":/etc");
    run(
        Command::new("virt-customize")
            .args(["--format", "qcow2", "-a"])
            .arg(&paths.disk)
            .arg("--copy-in")
            .arg(copy_source)
            .args(["--run-command", "chown -R 0:0 /etc/skel"]),
        flags,
        operation,
    )
}

pub(crate) fn create_iso(
    paths: &InstancePaths,
    seeds: &SeedFiles,
    flags: &Flags,
    operation: &OperationLog,
) -> Result<()> {
    run(
        Command::new("cloud-localds")
            .arg("-N")
            .arg(seeds.network_config())
            .arg(&paths.seed_iso)
            .arg(seeds.user_data())
            .arg(seeds.meta_data()),
        flags,
        operation,
    )
}

fn run(command: &mut Command, flags: &Flags, operation: &OperationLog) -> Result<()> {
    if flags.dry_run {
        operation.info(format!("Dry run: would run {command:?}"));
        return Ok(());
    }

    operation.info(format!("Running {command:?}"));
    let output = command
        .output()
        .with_context(|| format!("Failed to run {command:?}"))?;
    if !output.status.success() {
        bail!(
            "{command:?} failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    operation.info(format!("Command completed: {command:?}"));
    Ok(())
}

/// Only application-owned regular files may be exported or removed. The shared template is untouched.
pub(crate) fn check_instance_files(paths: &InstancePaths) -> Result<()> {
    for path in [&paths.disk, &paths.seed_iso] {
        match std::fs::symlink_metadata(path) {
            Ok(metadata) => anyhow::ensure!(
                metadata.is_file(),
                "Refusing non-regular instance file {}",
                path.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("Inspecting {}", path.display()));
            }
        }
    }
    Ok(())
}

pub(crate) fn delete_instance_files(
    paths: &InstancePaths,
    operation: &mut OperationLog,
) -> Result<()> {
    check_instance_files(paths)?;
    for (step, path) in [
        ("Deleting seed ISO", &paths.seed_iso),
        ("Deleting instance disk", &paths.disk),
    ] {
        operation.step(step);
        match std::fs::remove_file(path) {
            Ok(()) => operation.info(format!("Removed {}", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                operation.info("File already absent")
            }
            Err(error) => {
                return Err(error).with_context(|| format!("Removing {}", path.display()));
            }
        }
    }
    Ok(())
}

/// Removes only this export's private workspace on completion, failure, or client disconnect.
struct ExportWorkspace {
    directory: std::path::PathBuf,
    operation: OperationLog,
}

impl Drop for ExportWorkspace {
    fn drop(&mut self) {
        self.operation.info("Removing temporary disk export");
        if let Err(error) = std::fs::remove_dir_all(&self.directory) {
            self.operation.message(
                crate::logging::Severity::Warning,
                format!(
                    "Could not remove disk export {}: {error}",
                    self.directory.display()
                ),
            );
        }
    }
}

pub struct DiskExport {
    pub file: std::fs::File,
    pub size: u64,
    _workspace: ExportWorkspace,
}

pub(crate) fn export_disk(
    paths: &InstancePaths,
    export_root: &std::path::Path,
    operation: &mut OperationLog,
) -> Result<DiskExport> {
    use std::os::unix::fs::DirBuilderExt;
    check_instance_files(paths)?;
    std::fs::create_dir_all(export_root)?;
    let directory = export_root.join(format!("disk-{}", uuid::Uuid::new_v4()));
    std::fs::DirBuilder::new().mode(0o700).create(&directory)?;
    let workspace = ExportWorkspace {
        directory,
        operation: operation.clone(),
    };
    let destination = workspace.directory.join("disk.qcow2");
    operation.step("Exporting standalone disk image");
    // Omitting -B flattens the backing chain into an independently usable qcow2 image.
    run(
        Command::new("qemu-img")
            .args(["convert", "-f", "qcow2", "-O", "qcow2"])
            .arg(&paths.disk)
            .arg(&destination),
        &Flags {
            dry_run: false,
            update_image: false,
        },
        operation,
    )?;
    let file = std::fs::File::open(&destination)?;
    let size = file.metadata()?.len();
    operation.info("Completed: standalone disk ready for download");
    Ok(DiskExport {
        file,
        size,
        _workspace: workspace,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_failures_include_status_and_stderr() {
        let error = run(
            Command::new("sh").args(["-c", "printf 'image creation failed' >&2; exit 7"]),
            &Flags {
                dry_run: false,
                update_image: false,
            },
            &OperationLog::new(crate::logging::Logger::shared(), "test", None),
        )
        .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("exit status: 7"), "{message}");
        assert!(message.contains("image creation failed"), "{message}");
    }

    #[test]
    fn dry_run_does_not_launch_commands() {
        let logger = crate::logging::Logger::shared();
        run(
            &mut Command::new("/nonexistent/andromeda-test-command"),
            &Flags {
                dry_run: true,
                update_image: false,
            },
            &OperationLog::new(logger.clone(), "test", None),
        )
        .unwrap();
        let logger = logger.lock().unwrap();
        assert!(
            logger
                .entries()
                .iter()
                .any(|entry| entry.contains("Dry run: would run"))
        );
        assert!(
            logger
                .entries()
                .iter()
                .all(|entry| !entry.contains("Command completed"))
        );
    }
}
