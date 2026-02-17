use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::path::{PathBuf, Path};
use tauri::{State, Manager, Emitter};
use tokio::process::Command;
use regex::Regex;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use log::{info, error};

mod ffmpeg;
mod gpu;
mod logger;
mod error;

use ffmpeg::{FfmpegManager, ConversionProgress, FfmpegDownloader, FfmpegLocator, AdobePreset, get_adobe_presets, VIDEO_FORMATS, AUDIO_FORMATS, get_format_info};
use gpu::{GpuDetector, EncoderInfo, GpuInfo};
use error::AppError;

// Windows creation flag to hide console window
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

// State management
pub struct AppState {
    ffmpeg_manager: Arc<Mutex<FfmpegManager>>,
    ffmpeg_path: Arc<Mutex<Option<std::path::PathBuf>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            ffmpeg_manager: Arc::new(Mutex::new(FfmpegManager::new())),
            ffmpeg_path: Arc::new(Mutex::new(None)),
        }
    }
}

// Response structs for commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FfmpegStatus {
    pub available: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub source: Option<String>, // bundled, path, common, winget, downloaded
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: u64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportedFormats {
    pub video: Vec<String>,
    pub audio: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    pub name: String,
    pub logical_cores: usize,
}

#[derive(Debug, Deserialize)]
struct StartConversionArgs {
    #[serde(alias = "inputFile")]
    input_file: String,
    #[serde(alias = "outputFile")]
    output_file: String,
    encoder: String,
    #[serde(alias = "gpuIndex")]
    gpu_index: Option<u32>,
    #[serde(alias = "cpuThreads")]
    cpu_threads: Option<u32>,
    preset: String,
    #[serde(alias = "isAdobePreset")]
    is_adobe_preset: Option<bool>,
}

#[tauri::command]
fn get_log_file_path() -> Result<PathBuf, AppError> {
    logger::session_log_path()
        .cloned()
        .ok_or_else(|| AppError::Internal("Session log not initialized".to_string()))
}

#[tauri::command]
async fn get_log_file_content() -> Result<String, AppError> {
    let log_path = logger::session_log_path()
        .ok_or_else(|| AppError::Internal("Session log not initialized".to_string()))?;
    tokio::fs::read_to_string(log_path).await.map_err(|e| e.into())
}

#[tauri::command]
async fn clear_session_log() -> Result<(), AppError> {
    let log_path = logger::session_log_path()
        .ok_or_else(|| AppError::Internal("Session log not initialized".to_string()))?;
    tokio::fs::write(log_path, b"").await.map_err(|e| e.into())
}

#[tauri::command]
fn get_log_dir(app_handle: tauri::AppHandle) -> Result<PathBuf, AppError> {
    logger::logs_dir(&app_handle).map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
fn get_default_output_dir() -> Result<String, AppError> {
    let mut candidate_bases = Vec::new();

    // Prefer the system Videos folder (e.g. C:\Users\<user>\Videos)
    if let Some(videos) = dirs::video_dir() {
        candidate_bases.push(videos);
    }

    #[cfg(target_os = "windows")]
    if let Some(user_profile) = std::env::var_os("USERPROFILE") {
        candidate_bases.push(PathBuf::from(user_profile).join("Videos"));
    }

    if let Some(home) = dirs::home_dir() {
        candidate_bases.push(home.join("Videos"));
    }

    if candidate_bases.is_empty() {
        return Err(AppError::Internal("Could not determine a default output directory".to_string()));
    }

    let mut seen = HashSet::new();
    let mut errors = Vec::new();

    for base in candidate_bases {
        let dedupe_key = base.to_string_lossy().to_lowercase();
        if !seen.insert(dedupe_key) {
            continue;
        }

        let target = base.join("Dreamcodec Output");
        match std::fs::create_dir_all(&target) {
            Ok(_) => return Ok(target.to_string_lossy().to_string()),
            Err(e) => errors.push(format!("{} ({})", target.display(), e)),
        }
    }

    if errors.is_empty() {
        Err(AppError::Internal("Failed to create output directory for unknown reason".to_string()))
    } else {
        Err(AppError::Internal(format!(
            "Failed to create default output directory. Tried: {}",
            errors.join("; ")
        )))
    }
}

async fn detect_cpu_name() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        let mut ps_cmd = Command::new("powershell");
        ps_cmd.args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_Processor | Select-Object -First 1 -ExpandProperty Name",
        ]);
        ps_cmd.creation_flags(CREATE_NO_WINDOW);

        if let Ok(output) = ps_cmd.output().await {
            if output.status.success() {
                let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let output = Command::new("sh")
            .args(["-lc", "cat /proc/cpuinfo | grep -m1 'model name' | cut -d: -f2-"])
            .output()
            .await
            .ok()?;
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !name.is_empty() {
            return Some(name);
        }
    }

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
            .await
            .ok()?;
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !name.is_empty() {
            return Some(name);
        }
    }

    None
}

#[tauri::command]
async fn get_cpu_info() -> Result<CpuInfo, AppError> {
    let logical_cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(0);
    let name = detect_cpu_name()
        .await
        .unwrap_or_else(|| "Unknown CPU".to_string());

    Ok(CpuInfo {
        name,
        logical_cores,
    })
}


// Initialize and find FFmpeg on app start
async fn initialize_ffmpeg(state: &AppState) -> FfmpegStatus {
    // Try to find FFmpeg using the locator
    match FfmpegLocator::find_ffmpeg().await {
        Some(path) => {
            // Determine source
            let source = if path.to_string_lossy().contains("WinGet") {
                Some("winget".to_string())
            } else if path.to_string_lossy().contains("Dreamcodec") || 
                      path.to_string_lossy().contains("GPU-MKV-to-MP4-Converter") {
                Some("downloaded".to_string())
            } else if path.to_string_lossy().starts_with("C:\\ffmpeg") ||
                      path.to_string_lossy().contains("Program Files\\ffmpeg") {
                Some("common".to_string())
            } else if let Ok(exe_dir) = std::env::current_exe() {
                if let Some(parent) = exe_dir.parent() {
                    if path.parent() == Some(parent) {
                        Some("bundled".to_string())
                    } else {
                        Some("path".to_string())
                    }
                } else {
                    Some("path".to_string())
                }
            } else {
                Some("path".to_string())
            };

            // Get version
            let version = FfmpegLocator::get_version(&path).await;
            
            let path_str = path.to_string_lossy().to_string();
            
            // Store the path
            let mut ffmpeg_path = state.ffmpeg_path.lock().unwrap();
            *ffmpeg_path = Some(path);
            
            FfmpegStatus {
                available: true,
                path: Some(path_str),
                version,
                source,
            }
        }
        None => {
            FfmpegStatus {
                available: false,
                path: None,
                version: None,
                source: None,
            }
        }
    }
}

// Command: Check if FFmpeg is available (auto-detect)
#[tauri::command]
async fn check_ffmpeg(state: State<'_, AppState>) -> Result<FfmpegStatus, AppError> {
    let status = initialize_ffmpeg(&state).await;
    Ok(status)
}

// Command: Download FFmpeg
#[tauri::command]
async fn download_ffmpeg(app_handle: tauri::AppHandle, state: State<'_, AppState>) -> Result<String, AppError> {
    let window = app_handle.get_webview_window("main");
    
    let progress_callback = move |downloaded: u64, total: u64| {
        let percentage = if total > 0 {
            (downloaded as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        
        let progress = DownloadProgress {
            downloaded,
            total,
            percentage,
        };
        
        if let Some(ref win) = window {
            let _: Result<(), _> = win.emit("ffmpeg-download-progress", progress);
        }
    };

    let ffmpeg_path = FfmpegDownloader::download_and_extract_ffmpeg(progress_callback).await?;
    
    // Update state with the new path
    let mut state_path = state.ffmpeg_path.lock().map_err(|e| AppError::Internal(e.to_string()))?;
    *state_path = Some(ffmpeg_path.clone());
    
    Ok(ffmpeg_path.to_string_lossy().to_string())
}

// Get the FFmpeg path from state or auto-detect
async fn get_ffmpeg_path(state: &AppState) -> Result<PathBuf, AppError> {
    // First check if we have a stored path
    {
        let stored = state.ffmpeg_path.lock().map_err(|e| AppError::Internal(e.to_string()))?;
        if let Some(ref path) = *stored {
            if path.exists() {
                return Ok(path.clone());
            }
        }
    }

    // Try to auto-detect
    if let Some(path) = FfmpegLocator::find_ffmpeg().await {
        let mut stored = state.ffmpeg_path.lock().map_err(|e| AppError::Internal(e.to_string()))?;
        *stored = Some(path.clone());
        return Ok(path);
    }

    Err(AppError::Ffmpeg("FFmpeg not found. Please install FFmpeg or restart the application.".to_string()))
}

// Command: Get available GPU encoders
#[tauri::command]
async fn get_gpu_info(state: State<'_, AppState>) -> Result<GpuInfo, AppError> {
    let ffmpeg_path = get_ffmpeg_path(&state).await.ok().map(|p| p.to_string_lossy().to_string());

    let result = GpuDetector::detect_with_ffmpeg(ffmpeg_path.as_deref()).await;
    
    result.map_err(|e| {
        error!("Error detecting GPU: {}", e);
        AppError::Internal(e.to_string())
    })
}

// Command: Get available encoders from ffmpeg
#[tauri::command]
async fn get_available_encoders(state: State<'_, AppState>) -> Result<Vec<EncoderInfo>, AppError> {
    let ffmpeg_path = get_ffmpeg_path(&state).await?;
    GpuDetector::get_available_encoders(Some(&ffmpeg_path.to_string_lossy())).await
        .map_err(|e| AppError::Internal(e.to_string()))
}

// Command: Get FFmpeg version
#[tauri::command]
async fn get_ffmpeg_version(state: State<'_, AppState>) -> Result<String, AppError> {
    let ffmpeg_path = get_ffmpeg_path(&state).await?;
    let output = Command::new(&ffmpeg_path)
        .args(&["-version"])
        .output()
        .await
        .map_err(|e| AppError::Ffmpeg(format!("Failed to run FFmpeg: {}", e)))?;
    
    if !output.status.success() {
        return Err(AppError::Ffmpeg("FFmpeg returned error".to_string()));
    }
    
    let version = String::from_utf8_lossy(&output.stdout);
    Ok(version.lines().next().unwrap_or("Unknown version").to_string())
}

// Command: Start conversion
#[tauri::command]
async fn start_conversion(
    state: State<'_, AppState>,
    input_file: Option<String>,
    output_file: Option<String>,
    encoder: Option<String>,
    gpu_index: Option<u32>,
    preset: Option<String>,
    is_adobe_preset: Option<bool>,
    args: Option<StartConversionArgs>,
    payload: Option<StartConversionArgs>,
) -> Result<String, AppError> {
    let task_id = Uuid::new_v4().to_string();
    let resolved = if let Some(args) = args {
        args
    } else if let Some(payload) = payload {
        payload
    } else {
        StartConversionArgs {
            input_file: input_file.ok_or_else(|| AppError::Internal("Missing input_file".to_string()))?,
            output_file: output_file.ok_or_else(|| AppError::Internal("Missing output_file".to_string()))?,
            encoder: encoder.unwrap_or_else(|| "libx264".to_string()),
            gpu_index,
            cpu_threads: None,
            preset: preset.unwrap_or_else(|| "fast".to_string()),
            is_adobe_preset,
        }
    };
    let StartConversionArgs {
        input_file,
        output_file,
        encoder,
        gpu_index,
        cpu_threads,
        preset,
        is_adobe_preset,
    } = resolved;

    if !std::path::Path::new(&input_file).exists() {
        return Err(AppError::Io(format!("Input file not found: {}", input_file)));
    }

    let output_ext = Path::new(&output_file)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let format_info = get_format_info(&output_ext);

    if let Some(parent) = std::path::Path::new(&output_file).parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Io(format!("Failed to create output directory: {}", e)))?;
    }

    // Get FFmpeg path automatically
    let ffmpeg_path = get_ffmpeg_path(&state).await?;
    let ffmpeg_path_str = ffmpeg_path.to_string_lossy().to_string();

    if !format_info.supports_video && format_info.supports_audio {
        let mut cmd = Command::new(&ffmpeg_path);
        cmd.args(&["-hide_banner", "-i", &input_file]);
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let output = cmd.output()
            .await
            .map_err(|e| AppError::Ffmpeg(format!("Failed to probe input: {}", e)))?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        let info = ffmpeg::VideoInfo::parse(&stderr)?;
        if info.audio_streams.is_empty() {
            return Err(AppError::Ffmpeg("Input has no audio stream; cannot create audio-only output.".to_string()));
        }
    }
    
    let manager = state.ffmpeg_manager.clone();
    let mut manager = manager.lock().map_err(|e| AppError::Internal(e.to_string()))?;
    
    manager.start_conversion(
        task_id.clone(),
        input_file,
        output_file,
        ffmpeg_path_str,
        encoder,
        gpu_index,
        cpu_threads,
        preset,
        is_adobe_preset.unwrap_or(false),
    )?;
    
    Ok(task_id)
}

// Command: Get conversion progress
#[tauri::command]
async fn get_conversion_progress(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<Option<ConversionProgress>, AppError> {
    let manager = state.ffmpeg_manager.clone();
    let manager = manager.lock().map_err(|e| AppError::Internal(e.to_string()))?;
    
    Ok(manager.get_progress(&task_id))
}

// Command: Cancel conversion
#[tauri::command]
async fn cancel_conversion(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<(), AppError> {
    let manager = state.ffmpeg_manager.clone();
    let mut manager = manager.lock().map_err(|e| AppError::Internal(e.to_string()))?;
    
    manager.cancel_conversion(&task_id)
}

// Command: Get video duration
#[tauri::command]
async fn get_video_duration(state: State<'_, AppState>, input_file: String) -> Result<f64, AppError> {
    let ffmpeg_path = get_ffmpeg_path(&state).await?;
    let output = Command::new(&ffmpeg_path)
        .args(&["-i", &input_file])
        .output()
        .await
        .map_err(|e| AppError::Ffmpeg(format!("Failed to probe video: {}", e)))?;
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // Parse duration from FFmpeg output
    let duration_regex = Regex::new(r"Duration: (\d+):(\d+):(\d+\.\d+)").map_err(|e| AppError::Internal(e.to_string()))?;
    
    if let Some(captures) = duration_regex.captures(&stderr) {
        let hours: f64 = captures[1].parse().unwrap_or(0.0);
        let minutes: f64 = captures[2].parse().unwrap_or(0.0);
        let seconds: f64 = captures[3].parse().unwrap_or(0.0);
        
        let total_seconds = hours * 3600.0 + minutes * 60.0 + seconds;
        return Ok(total_seconds);
    }
    
    Err(AppError::Ffmpeg("Could not determine video duration".to_string()))
}

// Command: Get video streams info
#[tauri::command]
async fn get_video_info(state: State<'_, AppState>, input_file: String) -> Result<ffmpeg::VideoInfo, AppError> {
    let ffmpeg_path = get_ffmpeg_path(&state).await?;
    let output = Command::new(&ffmpeg_path)
        .args(&["-hide_banner", "-i", &input_file])
        .output()
        .await
        .map_err(|e| AppError::Ffmpeg(format!("Failed to probe video: {}", e)))?;
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    let info = ffmpeg::VideoInfo::parse(&stderr)?;
    Ok(info)
}

// Command: Get supported formats
#[tauri::command]
async fn get_supported_formats() -> Result<SupportedFormats, AppError> {
    Ok(SupportedFormats {
        video: VIDEO_FORMATS.iter().map(|s| s.to_string()).collect(),
        audio: AUDIO_FORMATS.iter().map(|s| s.to_string()).collect(),
    })
}

// Command: Get Adobe/After Effects presets
#[tauri::command]
async fn get_adobe_presets_list() -> Result<Vec<AdobePreset>, AppError> {
    Ok(get_adobe_presets())
}

// Command: Get format info
#[tauri::command]
async fn get_format_information(extension: String) -> Result<serde_json::Value, AppError> {
    let info = get_format_info(&extension);
    
    Ok(serde_json::json!({
        "container": info.container,
        "default_video_codec": info.default_video_codec,
        "default_audio_codec": info.default_audio_codec,
        "supports_video": info.supports_video,
        "supports_audio": info.supports_audio,
    }))
}

// Command: Check if encoder is available
#[tauri::command]
async fn check_encoder_available(state: State<'_, AppState>, encoder: String) -> Result<bool, AppError> {
    let ffmpeg_path = get_ffmpeg_path(&state).await?;
    Ok(gpu::is_encoder_available(&ffmpeg_path.to_string_lossy(), &encoder).await)
}

// Command: Open file location in file explorer
#[tauri::command]
async fn open_file_location(file_path: String) -> Result<(), AppError> {
    let path = std::path::Path::new(&file_path);

    if !path.exists() {
        return Err(AppError::Io(format!("File not found: {}", file_path)));
    }

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new("explorer")
            .args(["/select,", &file_path])
            .spawn()
            .map_err(|e| AppError::Internal(format!("Failed to open file location: {}", e)))?;
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("open")
            .args(["-R", &file_path])
            .spawn()
            .map_err(|e| AppError::Internal(format!("Failed to open file location: {}", e)))?;
    }

    #[cfg(target_os = "linux")]
    {
        // For Linux, just open the parent directory
        if let Some(parent) = path.parent() {
            use std::process::Command;
            Command::new("xdg-open")
                .arg(parent)
                .spawn()
                .map_err(|e| AppError::Internal(format!("Failed to open file location: {}", e)))?;
        }
    }

    Ok(())
}

#[tauri::command]
fn log_message(level: String, message: String) {
    match level.as_str() {
        "info" => info!("{}", message),
        "warn" => log::warn!("{}", message),
        "error" => error!("{}", message),
        _ => info!("{}", message),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(|app| {
            // Initialize logging
            if let Err(e) = logger::init_logging(&app.handle()) {
                eprintln!("Failed to initialize logger: {}", e);
            }

            // Set up panic hook
            let app_handle = app.handle().clone();
            std::panic::set_hook(Box::new(move |panic_info| {
                let payload = panic_info.payload().downcast_ref::<&str>().unwrap_or(&"");
                let location = panic_info.location().map(|l| l.to_string()).unwrap_or_else(|| "".to_string());
                error!("Panic occurred: payload='{}', location='{}'", payload, location);
                let _ = app_handle.emit("panic", (payload, location));
            }));

            // Ensure default output directory is created on app startup
            if let Err(e) = get_default_output_dir() {
                error!("Warning: Failed to create default output directory: {}", e);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                if let Ok(mut manager) = window.app_handle().state::<AppState>().ffmpeg_manager.lock() {
                    manager.cancel_all();
                }
            }
        })
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            check_ffmpeg,
            download_ffmpeg,
            get_cpu_info,
            get_gpu_info,
            get_available_encoders,
            get_ffmpeg_version,
            start_conversion,
            get_conversion_progress,
            cancel_conversion,
            get_video_duration,
            get_video_info,
            get_supported_formats,
            get_adobe_presets_list,
            get_format_information,
            check_encoder_available,
            get_default_output_dir,
            open_file_location,
            get_log_file_path,
            get_log_file_content,
            clear_session_log,
            get_log_dir,
            log_message,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
