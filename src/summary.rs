use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use crate::config::Config;
use crate::human::format_bytes;
use crate::top::TopEntry;

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RunStatus {
    Completed,
    CompletedWithErrors,
    Interrupted,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub root_size_bytes: u64,
    pub root_size_human: String,
    pub files_seen: u64,
    pub directories_seen: u64,
    pub files_emitted: u64,
    pub directories_emitted: u64,
    pub bytes_seen: u64,
    pub errors: u64,
    pub skipped_hidden: u64,
    pub skipped_reparse_points: u64,
    pub output_files: Vec<PathBuf>,
    pub top_entries: Vec<TopEntry>,
}

impl ScanSummary {
    pub fn empty() -> Self {
        Self {
            root_size_bytes: 0,
            root_size_human: "0 B".to_string(),
            files_seen: 0,
            directories_seen: 0,
            files_emitted: 0,
            directories_emitted: 0,
            bytes_seen: 0,
            errors: 0,
            skipped_hidden: 0,
            skipped_reparse_points: 0,
            output_files: Vec::new(),
            top_entries: Vec::new(),
        }
    }

    pub fn set_root_size(&mut self, bytes: u64) {
        self.root_size_bytes = bytes;
        self.root_size_human = format_bytes(bytes);
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SummaryDocument<'a> {
    status: RunStatus,
    started: String,
    ended: String,
    config: &'a Config,
    summary: &'a ScanSummary,
}

pub fn write_all(
    config: &Config,
    summary: &ScanSummary,
    status: RunStatus,
    started: time::OffsetDateTime,
    ended: time::OffsetDateTime,
) -> std::io::Result<()> {
    let started_text = rfc3339(started);
    let ended_text = rfc3339(ended);
    let document = SummaryDocument {
        status,
        started: started_text.clone(),
        ended: ended_text.clone(),
        config,
        summary,
    };

    let json_path = config.output_dir.join("summary.json");
    let json = serde_json::to_vec_pretty(&document)?;
    std::fs::write(json_path, json)?;

    let mut md = File::create(config.output_dir.join("summary.md"))?;
    writeln!(md, "# Directory Size Summary")?;
    writeln!(md)?;
    writeln!(md, "- Status: {:?}", status)?;
    writeln!(md, "- Root: {}", config.root.display())?;
    writeln!(md, "- Started: {started_text}")?;
    writeln!(md, "- Ended: {ended_text}")?;
    writeln!(md, "- Total size: {}", summary.root_size_human)?;
    writeln!(md, "- Files seen: {}", summary.files_seen)?;
    writeln!(md, "- Directories seen: {}", summary.directories_seen)?;
    writeln!(md, "- Report rows: {}", summary.files_emitted + summary.directories_emitted)?;
    writeln!(md, "- Recoverable errors: {}", summary.errors)?;
    writeln!(md, "- Skipped hidden entries: {}", summary.skipped_hidden)?;
    writeln!(md, "- Skipped reparse points: {}", summary.skipped_reparse_points)?;
    writeln!(md)?;
    writeln!(md, "## Top Entries")?;
    writeln!(md)?;
    writeln!(md, "| type | size | path |")?;
    writeln!(md, "|---|---:|---|")?;
    for entry in &summary.top_entries {
        writeln!(
            md,
            "| {:?} | {} | {} |",
            entry.kind,
            format_bytes(entry.size_bytes),
            entry.path.replace('|', "\\|")
        )?;
    }

    let metadata = serde_json::json!({
        "tool": "sizes.exe",
        "version": env!("CARGO_PKG_VERSION"),
        "status": status,
        "started": started_text,
        "ended": ended_text,
        "format": config.format,
        "maxOutputSize": config.max_output_size,
        "outputFiles": summary.output_files,
    });
    std::fs::write(
        config.output_dir.join("metadata.json"),
        serde_json::to_vec_pretty(&metadata)?,
    )?;

    Ok(())
}

fn rfc3339(value: time::OffsetDateTime) -> String {
    value
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown-time".to_string())
}
