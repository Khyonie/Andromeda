use std::{
    fs::File,
    io::{self, BufWriter, Write},
    path::Path,
    sync::{Arc, Mutex},
};

use chrono::Local;

pub type SharedLogger = Arc<Mutex<Logger>>;

pub struct Logger {
    log: Vec<String>,
}

#[derive(strum::Display)]
pub enum Severity {
    Info,
    Warning,
    Error,
    Fatal,
}

impl Logger {
    pub fn new() -> Self {
        Self { log: Vec::new() }
    }

    pub fn shared() -> SharedLogger {
        Arc::new(Mutex::new(Self::new()))
    }

    fn log(&mut self, severity: Severity, message: &str) {
        let time = Local::now().format("%a %b %e %T %Y");
        let entry = format!("[ {severity} ][{time}] {message}");
        println!("{entry}");
        self.log.push(entry);
    }

    pub(crate) fn entries(&self) -> &[String] {
        &self.log
    }
}

pub fn log_message(logger: &SharedLogger, severity: Severity, message: impl AsRef<str>) {
    // Recover the log buffer after a poisoned lock instead of silently losing messages.
    let mut logger = logger.lock().unwrap_or_else(|error| error.into_inner());
    logger.log(severity, message.as_ref());
}

pub fn finalize_log(logger: &SharedLogger, log_folder: &Path) -> Result<(), io::Error> {
    log_message(logger, Severity::Info, "Finalizing log");
    // Write buffered lines to LOG-YYYY-MM-DD-N.txt in the configured directory.
    let time = Local::now().format("%Y-%m-%d");

    // Create a unique log file
    let mut count = 1;
    loop {
        let log_path = log_folder.join(format!("LOG-{time}-{count}.txt"));
        if !log_path.exists() {
            log_message(
                logger,
                Severity::Info,
                format!("Writing log to {}", log_path.to_string_lossy()),
            );
            let logger = logger.lock().map_err(|e| io::Error::other(e.to_string()))?;
            let file = File::create(log_path)?;
            let mut writer = BufWriter::new(file);

            for line in logger.entries() {
                writeln!(writer, "{}", line)?;
            }

            writer.flush()?;
            break;
        }
        count += 1;
    }

    Ok(())
}

/// Tracks the current operation step for progress and failure messages.
#[derive(Clone)]
pub(crate) struct OperationLog {
    logger: SharedLogger,
    step: &'static str,
}

impl OperationLog {
    pub fn new(logger: SharedLogger, operation: &str, target: Option<&str>) -> Self {
        let log = Self {
            logger,
            step: "Request",
        };
        match target {
            Some(target) => log.info(format!("Request received: {operation} instance {target:?}")),
            None => log.info(format!("Request received: {operation} instances")),
        }
        log
    }

    pub fn step(&mut self, step: &'static str) {
        self.step = step;
        self.info(step);
    }

    pub fn info(&self, message: impl AsRef<str>) {
        self.message(Severity::Info, message);
    }

    pub fn failure(&self, severity: Severity, error: impl std::fmt::Display) {
        self.message(severity, format!("{} failed: {error}", self.step));
    }

    pub fn message(&self, severity: Severity, message: impl AsRef<str>) {
        log_message(&self.logger, severity, message);
    }
}
