use anyhow::Result;
use log::LevelFilter;
use log4rs::append::console::{ConsoleAppender, Target};
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;
use tauri::Manager;

/// The name of the log file.
pub const LOG_FILE_NAME: &str = "dreamcodec.log";

/// Initializes the logging system.
///
/// This function sets up a logger that writes to both a file and the console.
/// The log file is created in the app's log directory.
///
/// # Arguments
///
/// * `app_handle` - A handle to the Tauri application instance.
///
/// # Returns
///
/// * `Ok(())` if the logger was initialized successfully.
/// * `Err(anyhow::Error)` if there was an error initializing the logger.
pub fn init_logging(app_handle: &tauri::AppHandle) -> Result<()> {
    // Get the path to the app's log directory.
    let log_dir = app_handle.path().app_log_dir().expect("Failed to get app log dir");
    if !log_dir.exists() {
        std::fs::create_dir_all(&log_dir)?;
    }
    let log_file_path = log_dir.join(LOG_FILE_NAME);

    // Create a console appender.
    let stdout = ConsoleAppender::builder()
        .target(Target::Stdout)
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S)} [{l}] {t} - {m}{n}",
        )))
        .build();

    // Create a file appender.
    let file_appender = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S)} [{l}] {t} - {m}{n}",
        )))
        .build(log_file_path)?;

    // Create the logger configuration.
    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .appender(Appender::builder().build("file", Box::new(file_appender)))
        .build(
            Root::builder()
                .appender("stdout")
                .appender("file")
                .build(LevelFilter::Info),
        )?;

    // Initialize the logger.
    log4rs::init_config(config)?;

    Ok(())
}
