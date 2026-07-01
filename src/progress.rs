use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::tty::IsTty;

use crate::human::format_bytes;
use crate::summary::{RunStatus, ScanSummary};

pub struct Progress {
    enabled: bool,
    last: Instant,
    start: Instant,
    spinner: usize,
    output_dir: PathBuf,
}

impl Progress {
    pub fn new(no_color: bool, output_dir: PathBuf) -> Self {
        Self {
            enabled: !no_color && std::io::stderr().is_tty(),
            last: Instant::now(),
            start: Instant::now(),
            spinner: 0,
            output_dir,
        }
    }

    pub fn tick(&mut self, current: &std::path::Path, summary: &ScanSummary) {
        if !self.enabled || self.last.elapsed() < Duration::from_millis(250) {
            return;
        }
        self.last = Instant::now();
        self.spinner = (self.spinner + 1) % 4;
        let glyph = ["|", "/", "-", "\\"][self.spinner];
        let elapsed = self.start.elapsed().as_secs();
        eprint!(
            "\r{glyph} files={} dirs={} bytes={} elapsed={}s current={} out={}   ",
            summary.files_seen,
            summary.directories_seen,
            format_bytes(summary.bytes_seen),
            elapsed,
            current.display(),
            self.output_dir.display()
        );
        let _ = std::io::stderr().flush();
    }

    pub fn finish(&mut self, summary: &ScanSummary, status: RunStatus) {
        if self.enabled {
            eprintln!(
                "\r{:?}: files={} dirs={} bytes={} errors={}        ",
                status,
                summary.files_seen,
                summary.directories_seen,
                format_bytes(summary.bytes_seen),
                summary.errors
            );
        }
    }
}
