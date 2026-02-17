use anyhow::Result;
use log::LevelFilter;
use log4rs::append::console::{ConsoleAppender, Target};
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;
use std::path::PathBuf;
use std::sync::OnceLock;
use tauri::Manager;

/// Subdirectory inside the app log dir where per-session logs are stored.
const LOGS_FOLDER: &str = "logs";

/// Holds the path of the current session's log file so Tauri commands can read it.
static SESSION_LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Returns the path of the current session's log file (set during init).
pub fn session_log_path() -> Option<&'static PathBuf> {
    SESSION_LOG_PATH.get()
}

/// Returns the logs directory path.
pub fn logs_dir(app_handle: &tauri::AppHandle) -> Result<PathBuf> {
    let log_dir = app_handle
        .path()
        .app_log_dir()
        .expect("Failed to get app log dir");
    Ok(log_dir.join(LOGS_FOLDER))
}

/// Initializes the logging system.
///
/// Each application launch creates a new log file with a timestamp, e.g.
/// `logs/dreamcodec_2026-02-17_18-30-00.txt`. A console appender is also
/// configured for development.
pub fn init_logging(app_handle: &tauri::AppHandle) -> Result<()> {
    let logs_dir = logs_dir(app_handle)?;
    if !logs_dir.exists() {
        std::fs::create_dir_all(&logs_dir)?;
    }

    // Build a timestamp for the session file name.
    let now = std::time::SystemTime::now();
    let since_epoch = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = since_epoch.as_secs();
    // Convert to a rough date-time without pulling in chrono.
    let (y, mo, d, h, mi, s) = epoch_to_datetime(secs);
    let filename = format!(
        "dreamcodec_{:04}-{:02}-{:02}_{:02}-{:02}-{:02}.txt",
        y, mo, d, h, mi, s
    );

    let log_file_path = logs_dir.join(&filename);

    // Store the session path so Tauri commands can read it.
    let _ = SESSION_LOG_PATH.set(log_file_path.clone());

    // Console appender.
    let stdout = ConsoleAppender::builder()
        .target(Target::Stdout)
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S)} [{l}] {t} - {m}{n}",
        )))
        .build();

    // Per-session file appender.
    let file_appender = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S)} [{l}] {t} - {m}{n}",
        )))
        .build(&log_file_path)?;

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .appender(Appender::builder().build("file", Box::new(file_appender)))
        .build(
            Root::builder()
                .appender("stdout")
                .appender("file")
                .build(LevelFilter::Info),
        )?;

    log4rs::init_config(config)?;

    Ok(())
}

/// Minimal epoch-to-datetime conversion (UTC) to avoid adding a chrono dependency.
fn epoch_to_datetime(epoch: u64) -> (u64, u64, u64, u64, u64, u64) {
    let s = epoch % 60;
    let total_min = epoch / 60;
    let mi = total_min % 60;
    let total_hr = total_min / 60;
    let h = total_hr % 24;
    let mut days = total_hr / 24;

    // Walk years from 1970.
    let mut y = 1970u64;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        y += 1;
    }

    let leap = is_leap(y);
    let month_days: [u64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut mo = 0u64;
    for (i, &md) in month_days.iter().enumerate() {
        if days < md {
            mo = i as u64 + 1;
            break;
        }
        days -= md;
    }
    let d = days + 1;
    (y, mo, d, h, mi, s)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
