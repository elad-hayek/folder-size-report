use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::config::{Config, EntryKind, OutputFormat};
use crate::human::format_bytes;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportEntry {
    #[serde(rename = "type")]
    pub entry_type: EntryKind,
    pub path: String,
    pub size_human: String,
    pub size_bytes: u64,
}

impl ReportEntry {
    pub fn new(kind: EntryKind, path: &Path, size_bytes: u64) -> Self {
        Self {
            entry_type: kind,
            path: path.display().to_string(),
            size_human: format_bytes(size_bytes),
            size_bytes,
        }
    }
}

pub struct ReportWriter {
    output_dir: PathBuf,
    format: OutputFormat,
    max_bytes: u64,
    part_index: usize,
    current_bytes: u64,
    writer: BufWriter<File>,
    part_files: Vec<PathBuf>,
    md_header_written: bool,
    csv_header: Vec<u8>,
}

impl ReportWriter {
    pub fn create(config: &Config) -> std::io::Result<Self> {
        let first = part_path(&config.output_dir, 1, config.format);
        let file = File::create(&first)?;
        let mut writer = Self {
            output_dir: config.output_dir.clone(),
            format: config.format,
            max_bytes: config.max_output_size,
            part_index: 1,
            current_bytes: 0,
            writer: BufWriter::new(file),
            part_files: vec![first],
            md_header_written: false,
            csv_header: b"type,path,sizeHuman,sizeBytes\n".to_vec(),
        };
        writer.write_part_header()?;
        Ok(writer)
    }

    pub fn write(&mut self, entry: &ReportEntry) -> std::io::Result<Option<String>> {
        let row = self.serialize(entry)?;
        if self.current_bytes > self.header_len() as u64
            && self.current_bytes + row.len() as u64 > self.max_bytes
        {
            self.rotate()?;
        }

        let warning = if row.len() as u64 > self.max_bytes {
            Some(format!(
                "single row {} bytes exceeds max output size {}",
                row.len(),
                self.max_bytes
            ))
        } else {
            None
        };

        self.writer.write_all(&row)?;
        self.current_bytes += row.len() as u64;
        Ok(warning)
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }

    pub fn part_files(&self) -> &[PathBuf] {
        &self.part_files
    }

    fn rotate(&mut self) -> std::io::Result<()> {
        self.writer.flush()?;
        self.part_index += 1;
        let path = part_path(&self.output_dir, self.part_index, self.format);
        self.writer = BufWriter::new(File::create(&path)?);
        self.part_files.push(path);
        self.current_bytes = 0;
        self.md_header_written = false;
        self.write_part_header()
    }

    fn write_part_header(&mut self) -> std::io::Result<()> {
        match self.format {
            OutputFormat::Csv => {
                self.writer.write_all(&self.csv_header)?;
                self.current_bytes += self.csv_header.len() as u64;
            }
            OutputFormat::Md => {
                let header = b"| type | path | sizeHuman | sizeBytes |\n|---|---:|---:|---:|\n";
                self.writer.write_all(header)?;
                self.current_bytes += header.len() as u64;
                self.md_header_written = true;
            }
            OutputFormat::Jsonl | OutputFormat::Txt => {}
        }
        Ok(())
    }

    fn header_len(&self) -> usize {
        match self.format {
            OutputFormat::Csv => self.csv_header.len(),
            OutputFormat::Md => 60,
            OutputFormat::Jsonl | OutputFormat::Txt => 0,
        }
    }

    fn serialize(&self, entry: &ReportEntry) -> std::io::Result<Vec<u8>> {
        let row = match self.format {
            OutputFormat::Jsonl => {
                let mut row = serde_json::to_vec(entry)?;
                row.push(b'\n');
                row
            }
            OutputFormat::Csv => csv_row(entry),
            OutputFormat::Md => format!(
                "| {} | {} | {} | {} |\n",
                kind_text(entry.entry_type),
                md_escape(&entry.path),
                entry.size_human,
                entry.size_bytes
            )
            .into_bytes(),
            OutputFormat::Txt => format!(
                "type={} path={} sizeHuman={} sizeBytes={}\n",
                kind_text(entry.entry_type),
                entry.path,
                entry.size_human,
                entry.size_bytes
            )
            .into_bytes(),
        };
        Ok(row)
    }
}

fn part_path(output_dir: &Path, index: usize, format: OutputFormat) -> PathBuf {
    let ext = match format {
        OutputFormat::Jsonl => "jsonl",
        OutputFormat::Csv => "csv",
        OutputFormat::Md => "md",
        OutputFormat::Txt => "txt",
    };
    output_dir.join(format!("report.part-{index:03}.{ext}"))
}

fn kind_text(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::File => "file",
        EntryKind::Directory => "directory",
    }
}

fn csv_row(entry: &ReportEntry) -> Vec<u8> {
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(Vec::new());
    writer
        .write_record([
            kind_text(entry.entry_type),
            &entry.path,
            &entry.size_human,
            &entry.size_bytes.to_string(),
        ])
        .expect("csv write to memory cannot fail");
    writer.into_inner().expect("csv memory flush cannot fail")
}

fn md_escape(value: &str) -> String {
    value.replace('|', "\\|")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonl_split_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config {
            root: dir.path().to_path_buf(),
            depth: None,
            min_size: 0,
            types: [EntryKind::File, EntryKind::Directory].into_iter().collect(),
            output_dir: dir.path().join("out"),
            format: OutputFormat::Jsonl,
            max_output_size: 90,
            include_hidden: true,
            follow_symlinks: false,
            no_color: true,
            top: 10,
        };
        std::fs::create_dir_all(&config.output_dir).unwrap();
        let mut writer = ReportWriter::create(&config).unwrap();
        for idx in 0..3 {
            writer
                .write(&ReportEntry::new(
                    EntryKind::File,
                    Path::new(&format!("long-file-name-{idx}")),
                    100,
                ))
                .unwrap();
        }
        writer.flush().unwrap();
        assert!(writer.part_files().len() >= 2);
    }
}
