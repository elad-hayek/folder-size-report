use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use clap::{Parser, ValueEnum};

use crate::human::ByteSize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Jsonl,
    Csv,
    Md,
    Txt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    File,
    Directory,
}

#[derive(Parser, Debug)]
#[command(
    name = "sizes",
    bin_name = "sizes.exe",
    about = "Analyze recursive Windows directory sizes and write split reports.",
    after_help = "Examples:\n  sizes.exe C:\\Users\\me\\Downloads\n  sizes.exe . --format csv --min-size 10MB --top 50\n  sizes.exe D:\\Data --depth 3 --type file --output .\\size-report"
)]
struct Cli {
    /// Root directory to scan.
    root: PathBuf,

    /// Maximum directory depth to emit. Root depth is 0. Scan totals remain full-tree totals.
    #[arg(long)]
    depth: Option<usize>,

    /// Minimum entry size to emit, e.g. 10KB, 50MB, 1GB. Scan totals remain full-tree totals.
    #[arg(long, default_value = "0")]
    min_size: String,

    /// Entry type to emit. Repeat: --type file --type dir.
    #[arg(long = "type")]
    types: Vec<String>,

    /// Output directory. Defaults to .\size-report, or timestamped child when it already exists.
    #[arg(long, short = 'o')]
    output: Option<PathBuf>,

    /// Report stream format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Jsonl)]
    format: OutputFormat,

    /// Maximum bytes per report part, e.g. 50MB.
    #[arg(long, default_value = "50MB")]
    max_output_size: String,

    /// Include hidden files and directories in report rows and traversal.
    #[arg(long)]
    include_hidden: bool,

    /// Follow symlinks and junctions. Disabled by default to avoid loops.
    #[arg(long)]
    follow_symlinks: bool,

    /// Disable ANSI color/progress control.
    #[arg(long)]
    no_color: bool,

    /// Number of largest files/directories to retain in summaries.
    #[arg(long, default_value_t = 20)]
    top: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub root: PathBuf,
    pub depth: Option<usize>,
    pub min_size: u64,
    pub types: BTreeSet<EntryKind>,
    pub output_dir: PathBuf,
    pub format: OutputFormat,
    pub max_output_size: u64,
    pub include_hidden: bool,
    pub follow_symlinks: bool,
    pub no_color: bool,
    pub top: usize,
}

impl Config {
    pub fn from_args() -> Self {
        let cli = Cli::parse();
        Self::try_from_cli(cli).unwrap_or_else(|err| {
            eprintln!("error: {err}");
            std::process::exit(2);
        })
    }

    fn try_from_cli(cli: Cli) -> Result<Self, String> {
        let root = normalize_root(cli.root);

        if !root.exists() {
            return Err(format!("root does not exist: {}", root.display()));
        }
        if !root.is_dir() {
            return Err(format!("root is not a directory: {}", root.display()));
        }

        let min_size = ByteSize::parse(&cli.min_size)?.0;
        let max_output_size = ByteSize::parse(&cli.max_output_size)?.0;
        if max_output_size == 0 {
            return Err("--max-output-size must be greater than zero".to_string());
        }

        let mut types = BTreeSet::new();
        for value in &cli.types {
            match value.trim().to_ascii_lowercase().as_str() {
                "file" | "files" => {
                    types.insert(EntryKind::File);
                }
                "dir" | "dirs" | "directory" | "directories" => {
                    types.insert(EntryKind::Directory);
                }
                other => return Err(format!("invalid --type value: {other}")),
            }
        }
        if types.is_empty() {
            types.insert(EntryKind::File);
            types.insert(EntryKind::Directory);
        }

        Ok(Self {
            output_dir: choose_output_dir(cli.output.as_deref()),
            root,
            depth: cli.depth,
            min_size,
            types,
            format: cli.format,
            max_output_size,
            include_hidden: cli.include_hidden,
            follow_symlinks: cli.follow_symlinks,
            no_color: cli.no_color,
            top: cli.top,
        })
    }

    pub fn should_emit_kind(&self, kind: EntryKind) -> bool {
        self.types.contains(&kind)
    }
}

fn choose_output_dir(requested: Option<&Path>) -> PathBuf {
    let base = requested.map_or_else(|| PathBuf::from("size-report"), Path::to_path_buf);
    if !base.exists() {
        return base;
    }

    let stamp = time::OffsetDateTime::now_local()
        .unwrap_or_else(|_| time::OffsetDateTime::now_utc())
        .format(&time::macros::format_description!(
            "[year][month][day]-[hour][minute][second]"
        ))
        .unwrap_or_else(|_| "run".to_string());
    base.join(stamp)
}

fn normalize_root(root: PathBuf) -> PathBuf {
    let text = root.to_string_lossy();
    let bytes = text.as_bytes();
    if bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        PathBuf::from(format!("{}\\", text))
    } else {
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_type_values_parse() {
        let cli = Cli {
            root: std::env::current_dir().unwrap(),
            depth: None,
            min_size: "0".into(),
            types: vec!["file".into(), "dir".into()],
            output: None,
            format: OutputFormat::Jsonl,
            max_output_size: "1MB".into(),
            include_hidden: false,
            follow_symlinks: false,
            no_color: true,
            top: 10,
        };
        let config = Config::try_from_cli(cli).unwrap();
        assert!(config.types.contains(&EntryKind::File));
        assert!(config.types.contains(&EntryKind::Directory));
    }

    #[test]
    fn bad_type_fails() {
        let cli = Cli {
            root: std::env::current_dir().unwrap(),
            depth: None,
            min_size: "0".into(),
            types: vec!["banana".into()],
            output: None,
            format: OutputFormat::Jsonl,
            max_output_size: "1MB".into(),
            include_hidden: false,
            follow_symlinks: false,
            no_color: true,
            top: 10,
        };
        assert!(Config::try_from_cli(cli).is_err());
    }

    #[test]
    fn bare_drive_root_normalizes() {
        assert_eq!(normalize_root(PathBuf::from("C:")), PathBuf::from("C:\\"));
    }
}
