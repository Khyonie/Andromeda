use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::{Flags, paths::InstancePaths, seed::SeedFiles, server::log::OperationLog};

pub(crate) fn create_image(
    paths: &InstancePaths,
    size_mib: usize,
    flags: &Flags,
    operation: &OperationLog,
) -> Result<()> {
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
            &OperationLog::new(crate::server::log::Logger::shared(), "test", None),
        )
        .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("exit status: 7"), "{message}");
        assert!(message.contains("image creation failed"), "{message}");
    }

    #[test]
    fn dry_run_does_not_launch_commands() {
        let logger = crate::server::log::Logger::shared();
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
