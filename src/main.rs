mod config;
mod error_log;
mod human;
mod platform;
mod progress;
mod report;
mod scanner;
mod summary;
mod top;

use std::fs;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use config::Config;
use error_log::ErrorLog;
use progress::Progress;
use report::ReportWriter;
use scanner::Scanner;
use summary::RunStatus;

const EXIT_OK: i32 = 0;
const EXIT_SCAN_ERRORS: i32 = 5;
const EXIT_OUTPUT_FAILURE: i32 = 3;
const EXIT_INTERRUPTED: i32 = 4;

fn main() {
    let code = match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("output error: {err}");
            EXIT_OUTPUT_FAILURE
        }
    };
    std::process::exit(code);
}

fn run() -> std::io::Result<i32> {
    let config = Config::from_args();
    fs::create_dir_all(&config.output_dir)?;

    println!("output: {}", config.output_dir.display());

    let cancelled = Arc::new(AtomicBool::new(false));
    let ctrlc_flag = Arc::clone(&cancelled);
    ctrlc::set_handler(move || {
        ctrlc_flag.store(true, Ordering::SeqCst);
    })
    .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;

    let mut error_log = ErrorLog::create(&config.output_dir)?;
    let mut writer = ReportWriter::create(&config)?;
    let mut progress = Progress::new(config.no_color, config.output_dir.clone());

    let started = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    let mut scanner = Scanner::new(&config, &cancelled);
    let scan = scanner.scan(&mut writer, &mut error_log, &mut progress);
    let finish_write = writer.flush();

    let mut scan_result = match scan {
        Ok(result) => result,
        Err(err) => {
            eprintln!("scan failed: {err}");
            return Err(err);
        }
    };
    finish_write?;

    let ended = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    let interrupted = cancelled.load(Ordering::SeqCst);
    let status = if interrupted {
        RunStatus::Interrupted
    } else if scan_result.errors > 0 {
        RunStatus::CompletedWithErrors
    } else {
        RunStatus::Completed
    };

    scan_result.output_files = writer.part_files().to_vec();
    summary::write_all(&config, &scan_result, status, started, ended)?;
    error_log.flush()?;
    progress.finish(&scan_result, status);

    println!("done: {}", config.output_dir.display());

    Ok(match status {
        RunStatus::Completed => EXIT_OK,
        RunStatus::CompletedWithErrors => EXIT_SCAN_ERRORS,
        RunStatus::Interrupted => EXIT_INTERRUPTED,
    })
}
