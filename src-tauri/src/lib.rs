use std::sync::{Arc, Mutex};
use std::path::{PathBuf, Path};
use tauri::{State, Manager, Emitter};
use tokio::process::Command;
use regex::Regex;
use uuid::Uuid;
use serde::{Deserialize, Serialize};

mod ffmpeg;
mod gpu;

use ffmpeg::{FfmpegManager, ConversionProgress, FfmpegDownloader, FfmpegLocator, AdobePreset, get_adobe_presets, VIDEO_FORMATS, AUDIO_FORMATS, get_format_info};
use gpu::{GpuDetector, EncoderInfo, GpuInfo};

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

#[derive(Debug, Deserialize)]
struct StartConversionArgs {
    #[serde(alias = "inputFile")]
    input_file: String,
    #[serde(alias = "outputFile")]
    output_file: String,
    encoder: String,
    preset: String,
    #[serde(alias = "isAdobePreset")]
    is_adobe_preset: Option<bool>,
}

#[tauri::command]
fn get_default_output_dir() -> Result<String, String> {
    let base = dirs::document_dir()
        .map(|dir| dir.join("Videos"))
        .or_else(dirs::video_dir)
        .or_else(|| dirs::home_dir().map(|dir| dir.join("Videos")))
        .ok_or_else(|| "Could not determine a default output directory".to_string())?;

    let target = base.join("Dreamcodec Output");
    std::fs::create_dir_all(&target)
        .map_err(|e| format!("Failed to create output directory: {}", e))?;

    Ok(target.to_string_lossy().to_string())
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
async fn check_ffmpeg(state: State<'_, AppState>) -> Result<FfmpegStatus, String> {
    let status = initialize_ffmpeg(&state).await;
    Ok(status)
}

// Command: Download FFmpeg
#[tauri::command]
async fn download_ffmpeg(app_handle: tauri::AppHandle, state: State<'_, AppState>) -> Result<String, String> {
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
    let mut state_path = state.ffmpeg_path.lock().map_err(|e| e.to_string())?;
    *state_path = Some(ffmpeg_path.clone());
    
    Ok(ffmpeg_path.to_string_lossy().to_string())
}

// Get the FFmpeg path from state or auto-detect
async fn get_ffmpeg_path(state: &AppState) -> Result<PathBuf, String> {
    // First check if we have a stored path
    {
        let stored = state.ffmpeg_path.lock().map_err(|e| e.to_string())?;
        if let Some(ref path) = *stored {
            if path.exists() {
                return Ok(path.clone());
            }
        }
    }

    // Try to auto-detect
    if let Some(path) = FfmpegLocator::find_ffmpeg().await {
        let mut stored = state.ffmpeg_path.lock().map_err(|e| e.to_string())?;
        *stored = Some(path.clone());
        return Ok(path);
    }

    Err("FFmpeg not found. Please install FFmpeg or restart the application.".to_string())
}

// Command: Get available GPU encoders
#[tauri::command]
async fn get_gpu_info(state: State<'_, AppState>) -> Result<GpuInfo, String> {
    use std::fs::OpenOptions;
    use std::io::Write;

    let log_path = std::env::temp_dir().join("dreamcodec-debug.log");
    let log_write = |msg: &str| {
        let _ = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .and_then(|mut f| writeln!(f, "{}", msg));
        println!("{}", msg);
    };

    log_write("=== get_gpu_info called ===");

    // Try to get ffmpeg path
    let ffmpeg_path_result = get_ffmpeg_path(&state).await;
    log_write(&format!("FFmpeg path result: {:?}", ffmpeg_path_result));

    let ffmpeg_path = ffmpeg_path_result.ok().map(|p| {
        let path_str = p.to_string_lossy().to_string();
        log_write(&format!("Using FFmpeg at: {}", path_str));
        path_str
    });

    if ffmpeg_path.is_none() {
        log_write("WARNING: FFmpeg path is None, will try to use 'ffmpeg' command");
    }

    let result = GpuDetector::detect_with_ffmpeg(ffmpeg_path.as_deref()).await;
    log_write(&format!("GpuDetector result: {:?}", result));
    log_write(&format!("Log file: {}", log_path.display()));

    result.map_err(|e| {
        let err_msg = e.to_string();
        log_write(&format!("Error detecting GPU: {}", err_msg));
        err_msg
    })
}

// Command: Get available encoders from ffmpeg
#[tauri::command]
async fn get_available_encoders(state: State<'_, AppState>) -> Result<Vec<EncoderInfo>, String> {
    let ffmpeg_path = get_ffmpeg_path(&state).await?;
    GpuDetector::get_available_encoders(Some(&ffmpeg_path.to_string_lossy())).await
        .map_err(|e| e.to_string())
}

// Command: Get FFmpeg version
#[tauri::command]
async fn get_ffmpeg_version(state: State<'_, AppState>) -> Result<String, String> {
    let ffmpeg_path = get_ffmpeg_path(&state).await?;
    let output = Command::new(&ffmpeg_path)
        .args(&["-version"])
        .output()
        .await
        .map_err(|e| format!("Failed to run FFmpeg: {}", e))?;
    
    if !output.status.success() {
        return Err("FFmpeg returned error".to_string());
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
    preset: Option<String>,
    is_adobe_preset: Option<bool>,
    args: Option<StartConversionArgs>,
    payload: Option<StartConversionArgs>,
) -> Result<String, String> {
    let task_id = Uuid::new_v4().to_string();
    let resolved = if let Some(args) = args {
        args
    } else if let Some(payload) = payload {
        payload
    } else {
        StartConversionArgs {
            input_file: input_file.ok_or_else(|| "Missing input_file".to_string())?,
            output_file: output_file.ok_or_else(|| "Missing output_file".to_string())?,
            encoder: encoder.unwrap_or_else(|| "libx264".to_string()),
            preset: preset.unwrap_or_else(|| "fast".to_string()),
            is_adobe_preset,
        }
    };
    let StartConversionArgs {
        input_file,
        output_file,
        encoder,
        preset,
        is_adobe_preset,
    } = resolved;

    if !std::path::Path::new(&input_file).exists() {
        return Err(format!("Input file not found: {}", input_file));
    }

    let output_ext = Path::new(&output_file)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let format_info = get_format_info(&output_ext);

    if let Some(parent) = std::path::Path::new(&output_file).parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create output directory: {}", e))?;
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
            .map_err(|e| format!("Failed to probe input: {}", e))?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        let info = ffmpeg::VideoInfo::parse(&stderr)?;
        if info.audio_streams.is_empty() {
            return Err("Input has no audio stream; cannot create audio-only output.".to_string());
        }
    }

    println!(
        "start_conversion: input='{}' output='{}' encoder='{}' preset='{}' adobe={:?}",
        input_file, output_file, encoder, preset, is_adobe_preset
    );
    
    let manager = state.ffmpeg_manager.clone();
    let mut manager = manager.lock().map_err(|e| e.to_string())?;
    
    manager.start_conversion(
        task_id.clone(),
        input_file,
        output_file,
        ffmpeg_path_str,
        encoder,
        preset,
        is_adobe_preset.unwrap_or(false),
    ).map_err(|e| e.to_string())?;
    
    Ok(task_id)
}

// Command: Get conversion progress
#[tauri::command]
async fn get_conversion_progress(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<Option<ConversionProgress>, String> {
    let manager = state.ffmpeg_manager.clone();
    let manager = manager.lock().map_err(|e| e.to_string())?;
    
    Ok(manager.get_progress(&task_id))
}

// Command: Cancel conversion
#[tauri::command]
async fn cancel_conversion(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<(), String> {
    let manager = state.ffmpeg_manager.clone();
    let mut manager = manager.lock().map_err(|e| e.to_string())?;
    
    manager.cancel_conversion(&task_id).map_err(|e| e.to_string())
}

// Command: Get video duration
#[tauri::command]
async fn get_video_duration(state: State<'_, AppState>, input_file: String) -> Result<f64, String> {
    let ffmpeg_path = get_ffmpeg_path(&state).await?;
    let output = Command::new(&ffmpeg_path)
        .args(&["-i", &input_file])
        .output()
        .await
        .map_err(|e| format!("Failed to probe video: {}", e))?;
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // Parse duration from FFmpeg output
    let duration_regex = Regex::new(r"Duration: (\d+):(\d+):(\d+\.\d+)").unwrap();
    
    if let Some(captures) = duration_regex.captures(&stderr) {
        let hours: f64 = captures[1].parse().unwrap_or(0.0);
        let minutes: f64 = captures[2].parse().unwrap_or(0.0);
        let seconds: f64 = captures[3].parse().unwrap_or(0.0);
        
        let total_seconds = hours * 3600.0 + minutes * 60.0 + seconds;
        return Ok(total_seconds);
    }
    
    Err("Could not determine video duration".to_string())
}

// Command: Get video streams info
#[tauri::command]
async fn get_video_info(state: State<'_, AppState>, input_file: String) -> Result<ffmpeg::VideoInfo, String> {
    let ffmpeg_path = get_ffmpeg_path(&state).await?;
    let output = Command::new(&ffmpeg_path)
        .args(&["-hide_banner", "-i", &input_file])
        .output()
        .await
        .map_err(|e| format!("Failed to probe video: {}", e))?;
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    let info = ffmpeg::VideoInfo::parse(&stderr)?;
    Ok(info)
}

// Command: Get supported formats
#[tauri::command]
async fn get_supported_formats() -> Result<SupportedFormats, String> {
    Ok(SupportedFormats {
        video: VIDEO_FORMATS.iter().map(|s| s.to_string()).collect(),
        audio: AUDIO_FORMATS.iter().map(|s| s.to_string()).collect(),
    })
}

// Command: Get Adobe/After Effects presets
#[tauri::command]
async fn get_adobe_presets_list() -> Result<Vec<AdobePreset>, String> {
    Ok(get_adobe_presets())
}

// Command: Get format info
#[tauri::command]
async fn get_format_information(extension: String) -> Result<serde_json::Value, String> {
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
async fn check_encoder_available(state: State<'_, AppState>, encoder: String) -> Result<bool, String> {
    let ffmpeg_path = get_ffmpeg_path(&state).await?;
    Ok(gpu::is_encoder_available(&ffmpeg_path.to_string_lossy(), &encoder).await)
}

// Command: Open file location in file explorer
#[tauri::command]
async fn open_file_location(file_path: String) -> Result<(), String> {
    let path = std::path::Path::new(&file_path);

    if !path.exists() {
        return Err(format!("File not found: {}", file_path));
    }

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new("explorer")
            .args(["/select,", &file_path])
            .spawn()
            .map_err(|e| format!("Failed to open file location: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("open")
            .args(["-R", &file_path])
            .spawn()
            .map_err(|e| format!("Failed to open file location: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        // For Linux, just open the parent directory
        if let Some(parent) = path.parent() {
            use std::process::Command;
            Command::new("xdg-open")
                .arg(parent)
                .spawn()
                .map_err(|e| format!("Failed to open file location: {}", e))?;
        }
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(|_app| {
            // Ensure default output directory is created on app startup
            if let Err(e) = get_default_output_dir() {
                eprintln!("Warning: Failed to create default output directory: {}", e);
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
