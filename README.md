# Windows Directory Size Analyzer CLI

`sizes.exe` scans a directory tree, computes recursive directory sizes, and writes split report files plus machine and human summaries.

## Why Rust

Rust fits this tool because it builds a native Windows executable, has fast filesystem traversal, keeps memory use explicit, and gives strong error handling without a runtime dependency.

## Build

```powershell
cargo build --release
```

Binary:

```text
target\release\sizes.exe
```

## Run

```powershell
sizes.exe C:\Users\me\Downloads
sizes.exe . --format csv --min-size 10MB --top 50
sizes.exe D:\Data --depth 3 --type file --output .\size-report
```

Default output directory is `.\size-report\`. If it already exists, `sizes.exe` writes to a timestamped subdirectory.

## Options

- `root`: directory to scan
- `--depth N`: maximum emitted depth; scan totals stay full-tree totals
- `--min-size SIZE`: minimum emitted row size, like `10KB`, `50MB`, `1GB`
- `--type file|dir`: repeatable emitted row type filter
- `--output DIR`: output directory
- `--format jsonl|csv|md|txt`: report format; default `jsonl`
- `--max-output-size SIZE`: split threshold; default `50MB`
- `--include-hidden`: include hidden entries
- `--follow-symlinks`: follow symlinks and junctions
- `--no-color`: disable in-place progress UI
- `--top N`: largest entries kept in summary

Invalid arguments exit `2`. Output write failures exit `3`. Ctrl+C flushes partial output and exits `4`. Completed scans with recoverable filesystem errors exit `5`.

## Output

Every output directory contains:

- `report.part-001.<format>` and more parts as needed
- `summary.json`
- `summary.md`
- `metadata.json`
- `errors.log`

JSONL row:

```json
{"type":"file","path":"C:\\Data\\a.bin","sizeHuman":"10.0 MB","sizeBytes":10485760}
```

Warning example:

```text
warning: C:\Data\locked: Access is denied. (os error 5)
```

Progress example:

```text
/ files=1200 dirs=80 bytes=1.40 GB elapsed=3s current=C:\Data out=size-report
```

Summary excerpt:

```json
{
  "status": "completed",
  "summary": {
    "rootSizeHuman": "1.40 GB",
    "filesSeen": 1200,
    "directoriesSeen": 80,
    "errors": 0
  }
}
```

## Design Decisions

- Directories are emitted after children, so directory rows contain recursive file sums.
- Filters affect report rows only. Scan totals and directory recursive sizes stay true full-tree totals.
- Hidden entries are not emitted by default, but still scanned so total sizes stay true recursive totals. Use `--include-hidden` to emit hidden rows.
- Reparse points are skipped by default to avoid junction loops.
- Report rows are serialized before writing; if a row would cross split limit, writer rotates first. A single row bigger than limit is written and logged as a warning.
- Entries within each directory are sorted by path for deterministic output.

## Dependencies

- `clap`: CLI parsing
- `serde`, `serde_json`, `csv`: output formats
- `time`: timestamps
- `ctrlc`: Ctrl+C handling
- `crossterm`: terminal TTY detection
- `windows-sys`: Windows file attributes
- `tempfile`: tests

## Future Improvements

- Optional pre-count mode for true progress bars
- Lower-level Windows APIs for faster metadata reads
- Ignore rules
- Compressed report parts
- HTML report
- Parallel tuning flags
