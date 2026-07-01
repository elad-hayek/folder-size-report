use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::{Config, EntryKind};
use crate::error_log::ErrorLog;
use crate::platform;
use crate::progress::Progress;
use crate::report::{ReportEntry, ReportWriter};
use crate::summary::ScanSummary;
use crate::top::{TopEntry, TopN};

pub struct Scanner<'a> {
    config: &'a Config,
    cancelled: &'a AtomicBool,
    summary: ScanSummary,
    top: TopN,
}

impl<'a> Scanner<'a> {
    pub fn new(config: &'a Config, cancelled: &'a AtomicBool) -> Self {
        Self {
            config,
            cancelled,
            summary: ScanSummary::empty(),
            top: TopN::new(config.top),
        }
    }

    pub fn scan(
        &mut self,
        writer: &mut ReportWriter,
        errors: &mut ErrorLog,
        progress: &mut Progress,
    ) -> std::io::Result<ScanSummary> {
        let root = self.config.root.clone();
        let root_size = self.scan_dir(&root, 0, false, writer, errors, progress)?;
        self.summary.set_root_size(root_size);
        self.summary.errors = errors.count();
        self.summary.top_entries = self.top.sorted_desc();
        Ok(self.summary.clone())
    }

    fn scan_dir(
        &mut self,
        path: &Path,
        depth: usize,
        hidden_ancestor: bool,
        writer: &mut ReportWriter,
        errors: &mut ErrorLog,
        progress: &mut Progress,
    ) -> std::io::Result<u64> {
        if self.cancelled.load(Ordering::SeqCst) {
            return Ok(0);
        }

        self.summary.directories_seen += 1;
        progress.tick(path, &self.summary);

        let read_dir = match fs::read_dir(path) {
            Ok(read_dir) => read_dir,
            Err(err) => {
                errors.record("warning", path, "read_dir", &err)?;
                self.summary.errors = errors.count();
                return Ok(0);
            }
        };

        let mut entries = Vec::new();
        for entry_result in read_dir {
            match entry_result {
                Ok(entry) => entries.push(entry),
                Err(err) => {
                    errors.record("warning", path, "dir_entry", &err)?;
                    self.summary.errors = errors.count();
                }
            }
        }
        entries.sort_by_key(|entry| entry.path());

        let mut total = 0_u64;
        for entry in entries {
            if self.cancelled.load(Ordering::SeqCst) {
                break;
            }
            let entry_path = entry.path();
            let metadata = match fs::symlink_metadata(&entry_path) {
                Ok(metadata) => metadata,
                Err(err) => {
                    errors.record("warning", &entry_path, "metadata", &err)?;
                    self.summary.errors = errors.count();
                    continue;
                }
            };

            let is_hidden = hidden_ancestor || platform::is_hidden(&entry_path, &metadata);

            if !self.config.follow_symlinks && platform::is_reparse_point(&entry_path, &metadata) {
                self.summary.skipped_reparse_points += 1;
                continue;
            }

            if metadata.is_dir() {
                let size = self.scan_dir(
                    &entry_path,
                    depth + 1,
                    is_hidden,
                    writer,
                    errors,
                    progress,
                )?;
                total = total.saturating_add(size);
                self.emit_directory(&entry_path, depth + 1, size, is_hidden, writer, errors)?;
            } else if metadata.is_file() {
                let size = metadata.len();
                self.summary.files_seen += 1;
                self.summary.bytes_seen = self.summary.bytes_seen.saturating_add(size);
                total = total.saturating_add(size);
                self.emit_file(&entry_path, depth + 1, size, is_hidden, writer, errors)?;
            }
        }

        if depth == 0 {
            self.emit_directory(path, depth, total, hidden_ancestor, writer, errors)?;
        }
        Ok(total)
    }

    fn emit_file(
        &mut self,
        path: &Path,
        depth: usize,
        size: u64,
        hidden: bool,
        writer: &mut ReportWriter,
        errors: &mut ErrorLog,
    ) -> std::io::Result<()> {
        self.top.push(TopEntry {
            size_bytes: size,
            kind: EntryKind::File,
            path: path.display().to_string(),
        });
        if hidden && !self.config.include_hidden {
            self.summary.skipped_hidden += 1;
            return Ok(());
        }
        if self.should_emit(EntryKind::File, depth, size) {
            let entry = ReportEntry::new(EntryKind::File, path, size);
            if let Some(message) = writer.write(&entry)? {
                let err = std::io::Error::new(std::io::ErrorKind::Other, message);
                errors.record("warning", path, "output_split", &err)?;
            }
            self.summary.files_emitted += 1;
        }
        Ok(())
    }

    fn emit_directory(
        &mut self,
        path: &Path,
        depth: usize,
        size: u64,
        hidden: bool,
        writer: &mut ReportWriter,
        errors: &mut ErrorLog,
    ) -> std::io::Result<()> {
        self.top.push(TopEntry {
            size_bytes: size,
            kind: EntryKind::Directory,
            path: path.display().to_string(),
        });
        if hidden && !self.config.include_hidden {
            self.summary.skipped_hidden += 1;
            return Ok(());
        }
        if self.should_emit(EntryKind::Directory, depth, size) {
            let entry = ReportEntry::new(EntryKind::Directory, path, size);
            if let Some(message) = writer.write(&entry)? {
                let err = std::io::Error::new(std::io::ErrorKind::Other, message);
                errors.record("warning", path, "output_split", &err)?;
            }
            self.summary.directories_emitted += 1;
        }
        Ok(())
    }

    fn should_emit(&self, kind: EntryKind, depth: usize, size: u64) -> bool {
        self.config.should_emit_kind(kind)
            && self.config.depth.is_none_or(|max| depth <= max)
            && size >= self.config.min_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{OutputFormat};
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;

    fn test_config(root: PathBuf, out: PathBuf) -> Config {
        Config {
            root,
            depth: None,
            min_size: 0,
            types: [EntryKind::File, EntryKind::Directory].into_iter().collect(),
            output_dir: out,
            format: OutputFormat::Jsonl,
            max_output_size: 1024 * 1024,
            include_hidden: true,
            follow_symlinks: false,
            no_color: true,
            top: 10,
        }
    }

    #[test]
    fn nested_sizes_are_recursive() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        let child = root.join("child");
        fs::create_dir_all(&child).unwrap();
        fs::File::create(root.join("a.bin"))
            .unwrap()
            .write_all(&[1, 2, 3])
            .unwrap();
        fs::File::create(child.join("b.bin"))
            .unwrap()
            .write_all(&[4, 5])
            .unwrap();

        let out = dir.path().join("out");
        fs::create_dir_all(&out).unwrap();
        let config = test_config(root, out.clone());
        let cancelled = AtomicBool::new(false);
        let mut scanner = Scanner::new(&config, &cancelled);
        let mut writer = ReportWriter::create(&config).unwrap();
        let mut errors = ErrorLog::create(&out).unwrap();
        let mut progress = Progress::new(true, out);

        let summary = scanner
            .scan(&mut writer, &mut errors, &mut progress)
            .unwrap();
        assert_eq!(summary.root_size_bytes, 5);
        assert_eq!(summary.files_seen, 2);
        assert_eq!(summary.directories_seen, 2);
    }

    #[test]
    fn min_size_filters_rows_not_totals() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        fs::create_dir_all(&root).unwrap();
        fs::File::create(root.join("small.bin"))
            .unwrap()
            .write_all(&[1])
            .unwrap();

        let out = dir.path().join("out");
        fs::create_dir_all(&out).unwrap();
        let mut config = test_config(root, out.clone());
        config.min_size = 10;
        let cancelled = AtomicBool::new(false);
        let mut scanner = Scanner::new(&config, &cancelled);
        let mut writer = ReportWriter::create(&config).unwrap();
        let mut errors = ErrorLog::create(&out).unwrap();
        let mut progress = Progress::new(true, out);

        let summary = scanner
            .scan(&mut writer, &mut errors, &mut progress)
            .unwrap();
        assert_eq!(summary.root_size_bytes, 1);
        assert_eq!(summary.files_emitted, 0);
    }
}
