use std::sync::{Arc, Mutex};

use chrono::Local;

pub type SharedLogger = Arc<Mutex<Logger>>;

pub struct Logger {
    log: Vec<String>,
}

#[derive(strum::Display)]
pub enum Severity {
    INFO,
    WARNING,
    ERROR,
    FATAL,
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

    #[cfg(test)]
    pub(crate) fn entries(&self) -> &[String] {
        &self.log
    }
}

pub fn log_message(logger: &SharedLogger, severity: Severity, message: impl AsRef<str>) {
    // Recover the log buffer after a poisoned lock instead of silently losing messages.
    let mut logger = logger.lock().unwrap_or_else(|error| error.into_inner());
    logger.log(severity, message.as_ref());
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
        self.message(Severity::INFO, message);
    }

    pub fn failure(&self, severity: Severity, error: impl std::fmt::Display) {
        self.message(severity, format!("{} failed: {error}", self.step));
    }

    pub fn message(&self, severity: Severity, message: impl AsRef<str>) {
        log_message(&self.logger, severity, message);
    }
}
