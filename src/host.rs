//! Host-wide Linux resource readings, independent of guest-agent availability.
use std::{
    collections::HashMap,
    fs,
    os::unix::fs::MetadataExt,
    sync::Mutex,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::paths::StoragePaths;

#[derive(Serialize)]
pub struct CpuUsage {
    pub logical_cpus: usize,
    pub used_percent: Option<f64>,
}

#[derive(Serialize)]
pub struct MemoryUsage {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Serialize)]
pub struct StorageUsage {
    pub paths: Vec<String>,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Serialize)]
pub struct ConfiguredResources {
    pub instances: u64,
    pub vcpus: u64,
    pub memory_bytes: u64,
}

#[derive(Serialize)]
pub struct HostStatus {
    pub cpu: Option<CpuUsage>,
    pub memory: Option<MemoryUsage>,
    pub storage: Vec<StorageUsage>,
    pub configured: ConfiguredResources,
    pub warnings: Vec<String>,
    pub sampled_at: String,
}

struct CpuSample {
    total: u64,
    idle: u64,
    at: Instant,
}

#[derive(Default)]
struct CpuSampler {
    previous: Option<CpuSample>,
    percent: Option<f64>,
}

#[derive(Default)]
pub struct HostMetrics {
    cpu: Mutex<CpuSampler>,
}

impl HostMetrics {
    pub fn read(&self, paths: &StoragePaths, configured: ConfiguredResources) -> HostStatus {
        let mut warnings = Vec::new();
        let cpu = match self.read_cpu() {
            Ok(cpu) => Some(cpu),
            Err(_) => {
                *self.cpu.lock().unwrap_or_else(|e| e.into_inner()) = CpuSampler::default();
                warnings.push("Host CPU usage is unavailable.".into());
                None
            }
        };
        let memory = match read_memory() {
            Ok(memory) => Some(memory),
            Err(_) => {
                warnings.push("Host memory usage is unavailable.".into());
                None
            }
        };
        let mut storage: Vec<StorageUsage> = Vec::new();
        let mut devices = HashMap::new();
        for path in [
            paths.instances(),
            paths.templates(),
            paths.seeds(),
            paths.data_directory().to_owned(),
        ] {
            let reading = (|| -> Result<_> {
                let device = fs::metadata(&path)?.dev();
                let stat = rustix::fs::statvfs(&path)?;
                let bytes = |blocks: u64| {
                    blocks
                        .checked_mul(stat.f_frsize)
                        .context("Filesystem size overflow")
                };
                let used = stat
                    .f_blocks
                    .checked_sub(stat.f_bfree)
                    .context("Invalid filesystem counters")?;
                Ok((
                    device,
                    StorageUsage {
                        paths: vec![path.display().to_string()],
                        total_bytes: bytes(stat.f_blocks)?,
                        used_bytes: bytes(used)?,
                        available_bytes: bytes(stat.f_bavail)?,
                    },
                ))
            })();
            match reading {
                Ok((device, reading)) => {
                    if let Some(&index) = devices.get(&device) {
                        let existing: &mut StorageUsage = &mut storage[index];
                        existing.paths.extend(reading.paths);
                    } else {
                        devices.insert(device, storage.len());
                        storage.push(reading);
                    }
                }
                Err(_) => warnings.push(format!(
                    "Storage usage is unavailable for {}.",
                    path.display()
                )),
            }
        }
        HostStatus {
            cpu,
            memory,
            storage,
            configured,
            warnings,
            sampled_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn read_cpu(&self) -> Result<CpuUsage> {
        // Serialize sampling so simultaneous admin tabs cannot reverse sample order.
        let mut sampler = self.cpu.lock().unwrap_or_else(|e| e.into_inner());
        let contents = fs::read_to_string("/proc/stat")?;
        let mut lines = contents.lines();
        let mut fields = lines
            .next()
            .context("Missing CPU totals")?
            .split_whitespace();
        ensure!(fields.next() == Some("cpu"), "Missing CPU totals");
        // guest and guest_nice are already included in user and nice: do not double-count them.
        let ticks: Vec<u64> = fields.take(8).map(str::parse).collect::<Result<_, _>>()?;
        ensure!(ticks.len() == 8, "Incomplete CPU counters");
        let total = ticks
            .iter()
            .try_fold(0u64, |sum, value| sum.checked_add(*value))
            .context("CPU counter overflow")?;
        let idle = ticks[3]
            .checked_add(ticks[4])
            .context("Idle counter overflow")?;
        let logical_cpus = lines
            .filter(|line| {
                line.strip_prefix("cpu")
                    .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
            })
            .count();
        ensure!(logical_cpus > 0, "Missing CPU count");
        let sample = CpuSample {
            total,
            idle,
            at: Instant::now(),
        };
        if let Some(previous) = &sampler.previous {
            let elapsed = sample.at.duration_since(previous.at);
            if elapsed < Duration::from_millis(500) {
                return Ok(CpuUsage {
                    logical_cpus,
                    used_percent: sampler.percent,
                });
            }
            sampler.percent = if elapsed <= Duration::from_secs(30) {
                sample
                    .total
                    .checked_sub(previous.total)
                    .zip(sample.idle.checked_sub(previous.idle))
                    .filter(|(total, idle)| *total > 0 && idle <= total)
                    .map(|(total, idle)| (total - idle) as f64 / total as f64 * 100.0)
            } else {
                None
            };
        }
        sampler.previous = Some(sample);
        Ok(CpuUsage {
            logical_cpus,
            used_percent: sampler.percent,
        })
    }
}

fn read_memory() -> Result<MemoryUsage> {
    let contents = fs::read_to_string("/proc/meminfo")?;
    let value = |name: &str| -> Result<u64> {
        let line = contents
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .context("Missing memory counter")?;
        let mut parts = line.split_whitespace();
        let kib: u64 = parts.next().context("Missing memory value")?.parse()?;
        ensure!(parts.next() == Some("kB"), "Unexpected memory units");
        kib.checked_mul(1024).context("Memory counter overflow")
    };
    let total_bytes = value("MemTotal:")?;
    let available_bytes = value("MemAvailable:")?;
    let used_bytes = total_bytes
        .checked_sub(available_bytes)
        .context("Invalid available memory")?;
    Ok(MemoryUsage {
        total_bytes,
        available_bytes,
        used_bytes,
    })
}
