use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize)]
pub struct LoggedError {
    pub timestamp: String,
    pub severity: String,
    pub path: String,
    pub error_type: String,
    pub message: String,
    pub os_code: Option<i32>,
}

pub struct ErrorLog {
    writer: BufWriter<File>,
    count: u64,
}

impl ErrorLog {
    pub fn create(output_dir: &Path) -> std::io::Result<Self> {
        let path = output_dir.join("errors.log");
        let file = File::create(&path)?;
        Ok(Self {
            writer: BufWriter::new(file),
            count: 0,
        })
    }

    pub fn record(
        &mut self,
        severity: &str,
        path: &Path,
        error_type: &str,
        err: &std::io::Error,
    ) -> std::io::Result<()> {
        let entry = LoggedError {
            timestamp: timestamp(),
            severity: severity.to_string(),
            path: path.display().to_string(),
            error_type: error_type.to_string(),
            message: err.to_string(),
            os_code: err.raw_os_error(),
        };
        self.count += 1;
        eprintln!("warning: {}: {}", entry.path, entry.message);
        writeln!(
            self.writer,
            "{}\t{}\t{}\t{}\t{}\t{:?}",
            entry.timestamp,
            entry.severity,
            entry.error_type,
            entry.path,
            entry.message,
            entry.os_code
        )
    }

    pub fn count(&self) -> u64 {
        self.count
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

pub fn timestamp() -> String {
    time::OffsetDateTime::now_local()
        .unwrap_or_else(|_| time::OffsetDateTime::now_utc())
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown-time".to_string())
}
