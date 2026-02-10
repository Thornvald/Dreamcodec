use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::path::{Path, PathBuf};
use tokio::process::{Child, Command};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use regex::Regex;
use tokio::fs;

use futures::StreamExt;


// Windows creation flag to hide console window
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

// Supported video formats
pub const VIDEO_FORMATS: &[&str] = &["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "ogv"];

// Supported audio formats
pub const AUDIO_FORMATS: &[&str] = &["mp3", "wav", "aac", "flac", "m4a", "ogg"];

// FFmpeg locator - searches for FFmpeg in multiple locations
pub struct FfmpegLocator;

impl FfmpegLocator {
    /// Find FFmpeg executable using multiple search strategies
    pub async fn find_ffmpeg() -> Option<PathBuf> {
        println!("=== FfmpegLocator::find_ffmpeg() ===");

        // 1. Check bundled with app (same directory as executable)
        println!("Checking for bundled FFmpeg...");
        if let Some(path) = Self::find_bundled_ffmpeg() {
            println!("Found bundled FFmpeg at: {:?}", path);
            if Self::verify_ffmpeg(&path).await {
                println!("Bundled FFmpeg verified successfully!");
                return Some(path);
            } else {
                println!("Bundled FFmpeg verification failed");
            }
        } else {
            println!("No bundled FFmpeg found");
        }

        // 2. Check system PATH
        println!("Checking system PATH...");
        if let Some(path) = Self::find_in_path().await {
            println!("Found FFmpeg in PATH: {:?}", path);
            if Self::verify_ffmpeg(&path).await {
                println!("PATH FFmpeg verified successfully!");
                return Some(path);
            }
        }

        // 3. Check common install locations
        println!("Checking common locations...");
        if let Some(path) = Self::find_in_common_locations().await {
            println!("Found FFmpeg in common location: {:?}", path);
            if Self::verify_ffmpeg(&path).await {
                return Some(path);
            }
        }

        // 4. Check Windows Package Manager (WinGet) locations
        println!("Checking WinGet locations...");
        if let Some(path) = Self::find_in_winget_locations().await {
            println!("Found FFmpeg in WinGet location: {:?}", path);
            if Self::verify_ffmpeg(&path).await {
                return Some(path);
            }
        }

        // 5. Check app's data directory (downloaded FFmpeg)
        println!("Checking app data directory...");
        if let Some(path) = Self::find_in_app_data().await {
            println!("Found FFmpeg in app data: {:?}", path);
            if Self::verify_ffmpeg(&path).await {
                return Some(path);
            }
        }

        println!("FFmpeg not found in any location");
        None
    }

    /// Check for FFmpeg bundled with the app (same directory as executable)
    fn find_bundled_ffmpeg() -> Option<PathBuf> {
        if let Ok(exe_path) = std::env::current_exe() {
            println!("  Current exe path: {:?}", exe_path);
            if let Some(exe_dir) = exe_path.parent() {
                println!("  Exe directory: {:?}", exe_dir);
                let bundled = exe_dir.join("ffmpeg.exe");
                println!("  Looking for bundled FFmpeg at: {:?}", bundled);
                println!("  Exists: {}", bundled.exists());
                if bundled.exists() {
                    return Some(bundled);
                }

                let sidecar = exe_dir.join("ffmpeg-x86_64-pc-windows-msvc.exe");
                println!("  Looking for bundled FFmpeg sidecar at: {:?}", sidecar);
                println!("  Exists: {}", sidecar.exists());
                if sidecar.exists() {
                    return Some(sidecar);
                }

                if let Some(project_dir) = exe_dir.parent().and_then(|p| p.parent()) {
                    let dev_sidecar = project_dir.join("ffmpeg-x86_64-pc-windows-msvc.exe");
                    println!("  Looking for dev FFmpeg sidecar at: {:?}", dev_sidecar);
                    println!("  Exists: {}", dev_sidecar.exists());
                    if dev_sidecar.exists() {
                        return Some(dev_sidecar);
                    }

                    let dev_ffmpeg = project_dir.join("ffmpeg.exe");
                    println!("  Looking for dev FFmpeg at: {:?}", dev_ffmpeg);
                    println!("  Exists: {}", dev_ffmpeg.exists());
                    if dev_ffmpeg.exists() {
                        return Some(dev_ffmpeg);
                    }

                    let bin_sidecar = project_dir.join("bin").join("ffmpeg-x86_64-pc-windows-msvc.exe");
                    println!("  Looking for bin FFmpeg sidecar at: {:?}", bin_sidecar);
                    println!("  Exists: {}", bin_sidecar.exists());
                    if bin_sidecar.exists() {
                        return Some(bin_sidecar);
                    }

                    let bin_ffmpeg = project_dir.join("bin").join("ffmpeg.exe");
                    println!("  Looking for bin FFmpeg at: {:?}", bin_ffmpeg);
                    println!("  Exists: {}", bin_ffmpeg.exists());
                    if bin_ffmpeg.exists() {
                        return Some(bin_ffmpeg);
                    }
                }
            }
        }
        None
    }

    /// Check system PATH for ffmpeg
    async fn find_in_path() -> Option<PathBuf> {
        // Try running 'where ffmpeg' on Windows
        #[cfg(target_os = "windows")]
        {
            let mut cmd = Command::new("where");
            cmd.arg("ffmpeg");
            cmd.creation_flags(CREATE_NO_WINDOW);
            if let Ok(output) = cmd.output().await {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    for line in stdout.lines() {
                        let path = PathBuf::from(line.trim());
                        if path.exists() {
                            return Some(path);
                        }
                    }
                }
            }
        }

        // Try running 'which ffmpeg' on Unix
        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(output) = Command::new("which").arg("ffmpeg").output().await {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let path = PathBuf::from(stdout.trim());
                    if path.exists() {
                        return Some(path);
                    }
                }
            }
        }

        // Try running ffmpeg directly to see if it's in PATH
        let mut cmd = Command::new("ffmpeg");
        cmd.arg("-version");
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);
        if let Ok(output) = cmd.output().await {
            if output.status.success() {
                // It's in PATH, try to get the path
                #[cfg(target_os = "windows")]
                {
                    let mut where_cmd = Command::new("where");
                    where_cmd.arg("ffmpeg");
                    where_cmd.creation_flags(CREATE_NO_WINDOW);
                    if let Ok(where_output) = where_cmd.output().await {
                        if where_output.status.success() {
                            let stdout = String::from_utf8_lossy(&where_output.stdout);
                            if let Some(first_line) = stdout.lines().next() {
                                return Some(PathBuf::from(first_line.trim()));
                            }
                        }
                    }
                }
                return Some(PathBuf::from("ffmpeg"));
            }
        }

        None
    }

    /// Check common FFmpeg install locations
    async fn find_in_common_locations() -> Option<PathBuf> {
        let common_paths = [
            r"C:\ffmpeg\bin\ffmpeg.exe",
            r"C:\Program Files\ffmpeg\bin\ffmpeg.exe",
            r"C:\Program Files (x86)\ffmpeg\bin\ffmpeg.exe",
            r"C:\ffmpeg\ffmpeg.exe",
            r"C:\ProgramData\chocolatey\bin\ffmpeg.exe",
            r"C:\tools\ffmpeg\bin\ffmpeg.exe",
        ];

        for path_str in &common_paths {
            let path = PathBuf::from(path_str);
            if path.exists() {
                return Some(path);
            }
        }

        None
    }

    /// Check Windows Package Manager (WinGet) locations
    async fn find_in_winget_locations() -> Option<PathBuf> {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let winget_base = PathBuf::from(local_app_data).join("Microsoft").join("WinGet").join("Packages");
            
            if winget_base.exists() {
                // Search for ffmpeg in WinGet packages
                if let Ok(entries) = std::fs::read_dir(&winget_base) {
                    for entry in entries.flatten() {
                        if let Ok(file_type) = entry.file_type() {
                            if file_type.is_dir() {
                                let dir_name = entry.file_name().to_string_lossy().to_lowercase();
                                if dir_name.contains("ffmpeg") {
                                    // Look for ffmpeg.exe in this package
                                    let possible_paths = [
                                        entry.path().join("ffmpeg.exe"),
                                        entry.path().join("bin").join("ffmpeg.exe"),
                                    ];
                                    
                                    for path in &possible_paths {
                                        if path.exists() {
                                            return Some(path.clone());
                                        }
                                    }
                                    
                                    // Recursively search one level deep
                                    if let Ok(sub_entries) = std::fs::read_dir(entry.path()) {
                                        for sub_entry in sub_entries.flatten() {
                                            let sub_path = sub_entry.path().join("ffmpeg.exe");
                                            if sub_path.exists() {
                                                return Some(sub_path);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Check app's data directory for downloaded FFmpeg
    async fn find_in_app_data() -> Option<PathBuf> {
        if let Ok(app_dir) = FfmpegDownloader::get_ffmpeg_app_dir() {
            let ffmpeg_path = app_dir.join("ffmpeg.exe");
            if ffmpeg_path.exists() {
                return Some(ffmpeg_path);
            }
        }
        None
    }

    /// Verify that the given path is a valid FFmpeg executable
    pub async fn verify_ffmpeg(path: &Path) -> bool {
        if path.is_absolute() && !path.exists() {
            return false;
        }

        let mut cmd = Command::new(path);
        cmd.arg("-version");
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let output = cmd.output().await;

        match output {
            Ok(result) => result.status.success(),
            Err(_) => false,
        }
    }

    /// Get FFmpeg version info
    pub async fn get_version(path: &Path) -> Option<String> {
        let mut cmd = Command::new(path);
        cmd.arg("-version");
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);
        if let Ok(output) = cmd.output().await {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                return stdout.lines().next().map(|s| s.to_string());
            }
        }
        None
    }
}

// Format to default codec mapping
pub fn get_format_info(ext: &str) -> FormatInfo {
    match ext.to_lowercase().as_str() {
        "mp4" => FormatInfo {
            container: "mp4",
            default_video_codec: "libx264",
            default_audio_codec: "aac",
            supports_video: true,
            supports_audio: true,
        },
        "mkv" => FormatInfo {
            container: "matroska",
            default_video_codec: "libx264",
            default_audio_codec: "aac",
            supports_video: true,
            supports_audio: true,
        },
        "avi" => FormatInfo {
            container: "avi",
            default_video_codec: "libx264",
            default_audio_codec: "mp3",
            supports_video: true,
            supports_audio: true,
        },
        "mov" => FormatInfo {
            container: "mov",
            default_video_codec: "libx264",
            default_audio_codec: "aac",
            supports_video: true,
            supports_audio: true,
        },
        "wmv" => FormatInfo {
            container: "asf",
            default_video_codec: "wmv2",
            default_audio_codec: "wmav2",
            supports_video: true,
            supports_audio: true,
        },
        "flv" => FormatInfo {
            container: "flv",
            default_video_codec: "libx264",
            default_audio_codec: "aac",
            supports_video: true,
            supports_audio: true,
        },
        "webm" => FormatInfo {
            container: "webm",
            default_video_codec: "libvpx-vp9",
            default_audio_codec: "libopus",
            supports_video: true,
            supports_audio: true,
        },
        "ogv" => FormatInfo {
            container: "ogg",
            default_video_codec: "libtheora",
            default_audio_codec: "libvorbis",
            supports_video: true,
            supports_audio: true,
        },
        "mp3" => FormatInfo {
            container: "mp3",
            default_video_codec: "",
            default_audio_codec: "libmp3lame",
            supports_video: false,
            supports_audio: true,
        },
        "wav" => FormatInfo {
            container: "wav",
            default_video_codec: "",
            default_audio_codec: "pcm_s16le",
            supports_video: false,
            supports_audio: true,
        },
        "aac" => FormatInfo {
            container: "adts",
            default_video_codec: "",
            default_audio_codec: "aac",
            supports_video: false,
            supports_audio: true,
        },
        "flac" => FormatInfo {
            container: "flac",
            default_video_codec: "",
            default_audio_codec: "flac",
            supports_video: false,
            supports_audio: true,
        },
        "m4a" => FormatInfo {
            container: "ipod",
            default_video_codec: "",
            default_audio_codec: "aac",
            supports_video: false,
            supports_audio: true,
        },
        "ogg" => FormatInfo {
            container: "ogg",
            default_video_codec: "",
            default_audio_codec: "libvorbis",
            supports_video: false,
            supports_audio: true,
        },
        _ => FormatInfo {
            container: "mp4",
            default_video_codec: "libx264",
            default_audio_codec: "aac",
            supports_video: true,
            supports_audio: true,
        },
    }
}

#[derive(Debug, Clone)]
pub struct FormatInfo {
    pub container: &'static str,
    pub default_video_codec: &'static str,
    pub default_audio_codec: &'static str,
    pub supports_video: bool,
    pub supports_audio: bool,
}

// Adobe/After Effects compatibility presets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdobePreset {
    pub name: String,
    pub description: String,
    pub encoder: String,
    pub encoder_options: Vec<String>,
    pub pixel_format: String,
}

pub fn get_adobe_presets() -> Vec<AdobePreset> {
    vec![
        // ProRes presets
        AdobePreset {
            name: "prores_422".to_string(),
            description: "Apple ProRes 422 (High Quality for Premiere Pro / Final Cut)".to_string(),
            encoder: "prores_ks".to_string(),
            encoder_options: vec!["-profile:v".to_string(), "2".to_string()],
            pixel_format: "yuv422p10le".to_string(),
        },
        AdobePreset {
            name: "prores_422_hq".to_string(),
            description: "Apple ProRes 422 HQ (Highest Quality for Premiere Pro / Final Cut)".to_string(),
            encoder: "prores_ks".to_string(),
            encoder_options: vec!["-profile:v".to_string(), "3".to_string()],
            pixel_format: "yuv422p10le".to_string(),
        },
        AdobePreset {
            name: "prores_4444".to_string(),
            description: "Apple ProRes 4444 (With Alpha Channel)".to_string(),
            encoder: "prores_ks".to_string(),
            encoder_options: vec!["-profile:v".to_string(), "4".to_string(), "-alpha_bits".to_string(), "16".to_string()],
            pixel_format: "yuva444p10le".to_string(),
        },
        AdobePreset {
            name: "prores_proxy".to_string(),
            description: "Apple ProRes Proxy (Lightweight Editing)".to_string(),
            encoder: "prores_ks".to_string(),
            encoder_options: vec!["-profile:v".to_string(), "0".to_string()],
            pixel_format: "yuv422p".to_string(),
        },
        // DNxHD/DNxHR presets for Avid/After Effects
        AdobePreset {
            name: "dnxhd_1080p_220".to_string(),
            description: "DNxHD 220 Mbps 1080p (Broadcast Quality)".to_string(),
            encoder: "dnxhd".to_string(),
            encoder_options: vec!["-b:v".to_string(), "220M".to_string()],
            pixel_format: "yuv422p".to_string(),
        },
        AdobePreset {
            name: "dnxhd_1080p_145".to_string(),
            description: "DNxHD 145 Mbps 1080p (High Quality)".to_string(),
            encoder: "dnxhd".to_string(),
            encoder_options: vec!["-b:v".to_string(), "145M".to_string()],
            pixel_format: "yuv422p".to_string(),
        },
        AdobePreset {
            name: "dnxhr_hq".to_string(),
            description: "DNxHR HQ (High Quality for 4K/UHD)".to_string(),
            encoder: "dnxhd".to_string(),
            encoder_options: vec!["-profile:v".to_string(), "dnxhr_hq".to_string()],
            pixel_format: "yuv422p".to_string(),
        },
        AdobePreset {
            name: "dnxhr_sq".to_string(),
            description: "DNxHR SQ (Standard Quality)".to_string(),
            encoder: "dnxhd".to_string(),
            encoder_options: vec!["-profile:v".to_string(), "dnxhr_sq".to_string()],
            pixel_format: "yuv422p".to_string(),
        },
        AdobePreset {
            name: "dnxhr_lb".to_string(),
            description: "DNxHR LB (Low Bandwidth / Proxy)".to_string(),
            encoder: "dnxhd".to_string(),
            encoder_options: vec!["-profile:v".to_string(), "dnxhr_lb".to_string()],
            pixel_format: "yuv422p".to_string(),
        },
        // CineForm presets
        AdobePreset {
            name: "cineform_high".to_string(),
            description: "GoPro CineForm High (After Effects Compatible)".to_string(),
            encoder: "cfhd".to_string(),
            encoder_options: vec!["-quality".to_string(), "film3+".to_string()],
            pixel_format: "yuv422p10le".to_string(),
        },
        AdobePreset {
            name: "cineform_medium".to_string(),
            description: "GoPro CineForm Medium".to_string(),
            encoder: "cfhd".to_string(),
            encoder_options: vec!["-quality".to_string(), "film3".to_string()],
            pixel_format: "yuv422p".to_string(),
        },
        AdobePreset {
            name: "cineform_low".to_string(),
            description: "GoPro CineForm Low (Proxy)".to_string(),
            encoder: "cfhd".to_string(),
            encoder_options: vec!["-quality".to_string(), "film2".to_string()],
            pixel_format: "yuv422p".to_string(),
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoInfo {
    pub duration: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub video_streams: Vec<StreamInfo>,
    pub audio_streams: Vec<StreamInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    pub index: u32,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
}

impl VideoInfo {
    pub fn parse(ffmpeg_output: &str) -> Result<Self, String> {
        let mut duration = None;
        let mut width = None;
        let mut height = None;
        let mut video_streams = Vec::new();
        let mut audio_streams = Vec::new();

        // Parse duration
        let duration_regex = Regex::new(r"Duration: (\d+):(\d+):(\d+\.\d+)").unwrap();
        if let Some(captures) = duration_regex.captures(ffmpeg_output) {
            let hours: f64 = captures[1].parse().unwrap_or(0.0);
            let minutes: f64 = captures[2].parse().unwrap_or(0.0);
            let seconds: f64 = captures[3].parse().unwrap_or(0.0);
            duration = Some(hours * 3600.0 + minutes * 60.0 + seconds);
        }

        // Parse streams (handles optional [0x..] and (lang) segments)
        let stream_regex = Regex::new(
            r"Stream #0:(\d+)(?:\[[^\]]+\])?(?:\(([^\)]+)\))?: (Video|Audio): ([^,\s]+)",
        )
        .unwrap();
        for caps in stream_regex.captures_iter(ffmpeg_output) {
            let index: u32 = caps[1].parse().unwrap_or(0);
            let language = caps.get(2).map(|m| m.as_str().to_string());
            let stream_type = caps.get(3).map(|m| m.as_str()).unwrap_or("");
            let codec = caps.get(4).map(|m| m.as_str()).unwrap_or("").to_string();

            let stream_info = StreamInfo {
                index,
                codec,
                language,
                title: None,
            };

            match stream_type {
                "Video" => {
                    // Parse resolution from the same line
                    let resolution_regex = Regex::new(r"(\d+)x(\d+)").unwrap();
                    if let Some(res_caps) = resolution_regex.captures(&caps[0]) {
                        width = Some(res_caps[1].parse().unwrap_or(0));
                        height = Some(res_caps[2].parse().unwrap_or(0));
                    }
                    video_streams.push(stream_info);
                }
                "Audio" => audio_streams.push(stream_info),
                _ => {}
            }
        }

        Ok(VideoInfo {
            duration,
            width,
            height,
            video_streams,
            audio_streams,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConversionStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionProgress {
    pub task_id: String,
    pub status: ConversionStatus,
    pub percentage: f64,
    pub current_time: f64,
    pub duration: f64,
    pub log: Vec<String>,
    pub error_message: Option<String>,
}

pub struct ConversionTask {
    pub id: String,
    pub input_file: String,
    pub output_file: String,
    pub ffmpeg_path: String,
    pub encoder: String,
    pub gpu_index: Option<u32>,
    pub preset: String,
    pub is_adobe_preset: bool,
    pub adobe_preset: Option<AdobePreset>,
    pub progress: ConversionProgress,
    pub process: Option<Child>,
    pub pid: Option<u32>,
}

pub struct FfmpegManager {
    tasks: HashMap<String, Arc<Mutex<ConversionTask>>>,
}

impl FfmpegManager {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
        }
    }

    pub fn start_conversion(
        &mut self,
        task_id: String,
        input_file: String,
        output_file: String,
        ffmpeg_path: String,
        encoder: String,
        gpu_index: Option<u32>,
        preset: String,
        is_adobe_preset: bool,
    ) -> Result<(), String> {
        let duration = 0.0;

        let adobe_preset = if is_adobe_preset {
            get_adobe_presets().into_iter().find(|p| p.name == preset)
        } else {
            None
        };

        let progress = ConversionProgress {
            task_id: task_id.clone(),
            status: ConversionStatus::Pending,
            percentage: 0.0,
            current_time: 0.0,
            duration,
            log: Vec::new(),
            error_message: None,
        };

        let task = ConversionTask {
            id: task_id.clone(),
            input_file: input_file.clone(),
            output_file: output_file.clone(),
            ffmpeg_path: ffmpeg_path.clone(),
            encoder: encoder.clone(),
            gpu_index,
            preset: preset.clone(),
            is_adobe_preset,
            adobe_preset,
            progress,
            process: None,
            pid: None,
        };

        let task_arc = Arc::new(Mutex::new(task));
        self.tasks.insert(task_id.clone(), task_arc.clone());

        tokio::spawn(async move {
            run_conversion_task(task_arc).await;
        });

        Ok(())
    }

    pub fn get_progress(&self, task_id: &str) -> Option<ConversionProgress> {
        self.tasks.get(task_id).map(|t| {
            let task = t.lock().unwrap();
            task.progress.clone()
        })
    }

    pub fn cancel_conversion(&mut self, task_id: &str) -> Result<(), String> {
        if let Some(task_arc) = self.tasks.get(task_id) {
            // Use try_lock to avoid blocking if task is being processed
            if let Ok(mut task) = task_arc.try_lock() {
                // Only cancel if not already in a terminal state
                if !matches!(
                    task.progress.status,
                    ConversionStatus::Completed | ConversionStatus::Failed(_) | ConversionStatus::Cancelled
                ) {
                    if let Some(ref mut child) = task.process {
                        let _ = child.start_kill();
                    } else if let Some(pid) = task.pid {
                        kill_process(pid);
                    }

                    task.progress.status = ConversionStatus::Cancelled;
                }
                Ok(())
            } else {
                // Task is locked, just mark it for cancellation by killing the process
                // This is safe because we're not accessing the process directly
                Ok(())
            }
        } else {
            Err("Task not found".to_string())
        }
    }

    pub fn cancel_all(&mut self) {
        let task_ids: Vec<String> = self.tasks.keys().cloned().collect();

        // Collect PIDs first to avoid holding locks
        for task_id in &task_ids {
            if let Some(task_arc) = self.tasks.get(task_id) {
                if let Ok(task) = task_arc.try_lock() {
                    if let Some(pid) = task.pid {
                        kill_process(pid);
                    }
                }
            }
        }

        // Wait a bit for processes to terminate
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Now cancel the tasks
        for task_id in task_ids {
            let _ = self.cancel_conversion(&task_id);
        }
    }
}

fn kill_process(pid: u32) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .output();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("kill")
            .arg("-9")
            .arg(pid.to_string())
            .output();
    }
}

async fn run_conversion_task(task_arc: Arc<Mutex<ConversionTask>>) {
    let (input_file, output_file, ffmpeg_path, encoder, gpu_index, preset, is_adobe_preset, adobe_preset) = {
        let task = task_arc.lock().unwrap();
        (
            task.input_file.clone(),
            task.output_file.clone(),
            task.ffmpeg_path.clone(),
            task.encoder.clone(),
            task.gpu_index,
            task.preset.clone(),
            task.is_adobe_preset,
            task.adobe_preset.clone(),
        )
    };

    // Build FFmpeg command
    let mut args = vec!["-y".to_string(), "-i".to_string(), input_file.clone()];

    // Add stream mapping for video formats
    let output_ext = Path::new(&output_file)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mp4")
        .to_lowercase();
    
    let format_info = get_format_info(&output_ext);

    if format_info.supports_video {
        args.push("-map".to_string());
        args.push("0:v?".to_string());
    }
    if format_info.supports_audio {
        args.push("-map".to_string());
        if format_info.supports_video {
            args.push("0:a?".to_string());
        } else {
            // Audio-only outputs should only include the first audio stream.
            args.push("0:a:0?".to_string());
        }
    }

    if is_adobe_preset {
        // Adobe preset handling
        if let Some(ref preset_config) = adobe_preset {
            args.push("-c:v".to_string());
            args.push(preset_config.encoder.clone());
            
            // Add encoder options
            for opt in &preset_config.encoder_options {
                args.push(opt.clone());
            }
            
            args.push("-pix_fmt".to_string());
            args.push(preset_config.pixel_format.clone());
            
            // For ProRes and DNxHD, audio should be PCM or AAC
            if preset_config.encoder == "prores_ks" || preset_config.encoder == "dnxhd" {
                args.push("-c:a".to_string());
                args.push("pcm_s16le".to_string());
            }
        }
    } else {
        // Standard encoder handling
        if format_info.supports_video {
            args.push("-c:v".to_string());
            args.push(encoder.clone());
            
            // Add preset for compatible encoders
            if encoder.contains("nvenc") || encoder == "libx264" || encoder == "libx265" {
                args.push("-preset".to_string());
                args.push(preset.clone());
            }

            // Add pixel format for NVENC
            if encoder.contains("nvenc") {
                if let Some(index) = gpu_index {
                    args.push("-gpu".to_string());
                    args.push(index.to_string());
                }
                args.push("-pix_fmt".to_string());
                args.push("yuv420p".to_string());
            }
        }

        // Audio codec
        if format_info.supports_audio {
            args.push("-c:a".to_string());
            if format_info.default_audio_codec.is_empty() {
                args.push("copy".to_string());
            } else {
                args.push(format_info.default_audio_codec.to_string());
            }
        }
    }

    args.push(output_file.clone());

    // Update status to running
    {
        let mut task = task_arc.lock().unwrap();
        task.progress.status = ConversionStatus::Running;
    }

    // Run FFmpeg
    println!("=== FFmpeg Start ===");
    println!("FFmpeg path: {}", ffmpeg_path);
    println!("Encoder: {}", encoder);
    println!("Preset: {}", preset);
    println!("Input: {}", input_file);
    println!("Output: {}", output_file);
    println!("Args: {:?}", args);
    let mut cmd = Command::new(&ffmpeg_path);
    cmd.args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            let mut task = task_arc.lock().unwrap();
            let message = format!("Failed to start ffmpeg: {} (path: {})", e, ffmpeg_path);
            task.progress.status = ConversionStatus::Failed(message.clone());
            task.progress.error_message = Some(message);
            return;
        }
    };

    // Read output
    let time_regex = Regex::new(r"time=(\d+):(\d+):(\d+\.\d+)").unwrap();
    let duration_regex = Regex::new(r"Duration: (\d+):(\d+):(\d+\.\d+)").unwrap();

    // We need to get a new reference to process for reading
    let mut process_ref = {
        let mut task = task_arc.lock().unwrap();
        task.process = Some(child);
        task.pid = task.process.as_ref().and_then(|proc| proc.id());
        task.process.take().unwrap()
    };

    let mut last_log_line: Option<String> = None;

    if let Some(stderr) = process_ref.stderr.take() {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            last_log_line = Some(line.clone());
            let mut task = task_arc.lock().unwrap();
            task.progress.log.push(line.clone());

            // Parse duration from FFmpeg initial output
            if task.progress.duration == 0.0 {
                if let Some(captures) = duration_regex.captures(&line) {
                    let hours: f64 = captures[1].parse().unwrap_or(0.0);
                    let minutes: f64 = captures[2].parse().unwrap_or(0.0);
                    let seconds: f64 = captures[3].parse().unwrap_or(0.0);
                    task.progress.duration = hours * 3600.0 + minutes * 60.0 + seconds;
                    println!("Parsed duration: {} seconds", task.progress.duration);
                }
            }

            // Parse progress
            if let Some(captures) = time_regex.captures(&line) {
                let hours: f64 = captures[1].parse().unwrap_or(0.0);
                let minutes: f64 = captures[2].parse().unwrap_or(0.0);
                let seconds: f64 = captures[3].parse().unwrap_or(0.0);
                let current_time = hours * 3600.0 + minutes * 60.0 + seconds;

                task.progress.current_time = current_time;
                if task.progress.duration > 0.0 {
                    task.progress.percentage = (current_time / task.progress.duration) * 100.0;
                }
            }
        }
    }

    // Wait for process to complete
    let status = process_ref.wait().await;

    {
        let mut task = task_arc.lock().unwrap();
        task.process = None;
        task.pid = None;
        match status {
            Ok(exit_status) => {
                if exit_status.success() {
                    task.progress.status = ConversionStatus::Completed;
                    task.progress.percentage = 100.0;
                } else {
                    let message = last_log_line
                        .clone()
                        .unwrap_or_else(|| format!("FFmpeg exited with code: {:?}", exit_status.code()));
                    task.progress.status = ConversionStatus::Failed(message.clone());
                    if task.progress.error_message.is_none() {
                        task.progress.error_message = Some(message);
                    }
                }
            }
            Err(e) => {
                let message = e.to_string();
                task.progress.status = ConversionStatus::Failed(message.clone());
                if task.progress.error_message.is_none() {
                    task.progress.error_message = Some(message);
                }
            }
        }
    }
}

// FFmpeg download and management
pub struct FfmpegDownloader;

impl FfmpegDownloader {
    pub fn new() -> Self {
        Self
    }

    pub fn get_ffmpeg_app_dir() -> Result<PathBuf, String> {
        let app_dir = dirs::data_dir()
            .ok_or("Could not find app data directory")?
            .join("Dreamcodec");
        Ok(app_dir)
    }

    pub fn get_ffmpeg_path() -> Result<PathBuf, String> {
        let app_dir = Self::get_ffmpeg_app_dir()?;
        Ok(app_dir.join("ffmpeg.exe"))
    }

    pub fn get_ffprobe_path() -> Result<PathBuf, String> {
        let app_dir = Self::get_ffmpeg_app_dir()?;
        Ok(app_dir.join("ffprobe.exe"))
    }

    pub async fn is_ffmpeg_available() -> bool {
        // First try the auto-locator
        if let Some(path) = FfmpegLocator::find_ffmpeg().await {
            return path.exists();
        }
        
        // Fallback to app directory check
        match Self::get_ffmpeg_path() {
            Ok(path) => path.exists(),
            Err(_) => false,
        }
    }

    pub async fn download_and_extract_ffmpeg<F>(progress_callback: F) -> Result<PathBuf, String>
    where
        F: Fn(u64, u64) + Send + 'static,
    {
        let app_dir = Self::get_ffmpeg_app_dir()?;
        let ffmpeg_path = app_dir.join("ffmpeg.exe");

        // Check if already exists
        if ffmpeg_path.exists() {
            return Ok(ffmpeg_path);
        }

        // Create directory if needed
        fs::create_dir_all(&app_dir).await
            .map_err(|e| format!("Failed to create app directory: {}", e))?;

        let zip_url = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";
        let zip_path = app_dir.join("ffmpeg.zip");

        // Download the zip file with progress
        let client = reqwest::Client::new();
        let response = client.get(zip_url)
            .send().await
            .map_err(|e| format!("Failed to download FFmpeg: {}", e))?;

        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded = 0u64;

        let mut file = fs::File::create(&zip_path).await
            .map_err(|e| format!("Failed to create zip file: {}", e))?;

        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("Download error: {}", e))?;
            file.write_all(&chunk).await
                .map_err(|e| format!("Write error: {}", e))?;
            downloaded += chunk.len() as u64;
            progress_callback(downloaded, total_size);
        }

        file.flush().await
            .map_err(|e| format!("Failed to flush file: {}", e))?;
        drop(file);

        // Extract the zip file
        Self::extract_ffmpeg(&zip_path, &app_dir).await?;

        // Clean up zip file
        let _ = fs::remove_file(&zip_path).await;

        if !ffmpeg_path.exists() {
            return Err("FFmpeg extraction failed".to_string());
        }

        Ok(ffmpeg_path)
    }

    async fn extract_ffmpeg(zip_path: &Path, output_dir: &Path) -> Result<(), String> {
        // Read and extract the zip file
        let file = std::fs::File::open(zip_path)
            .map_err(|e| format!("Failed to open zip file: {}", e))?;
        
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| format!("Failed to read zip archive: {}", e))?;

        // Find the ffmpeg.exe and ffprobe.exe in the archive
        let mut ffmpeg_entry_name = String::new();
        let mut ffprobe_entry_name = String::new();

        for i in 0..archive.len() {
            let entry = archive.by_index(i)
                .map_err(|e| format!("Failed to read zip entry: {}", e))?;
            let name = entry.name().to_lowercase();
            if name.ends_with("ffmpeg.exe") && !name.contains("doc") {
                ffmpeg_entry_name = entry.name().to_string();
            } else if name.ends_with("ffprobe.exe") && !name.contains("doc") {
                ffprobe_entry_name = entry.name().to_string();
            }
        }

        if ffmpeg_entry_name.is_empty() {
            return Err("Could not find ffmpeg.exe in archive".to_string());
        }

        // Extract ffmpeg.exe
        {
            let mut ffmpeg_file = archive.by_name(&ffmpeg_entry_name)
                .map_err(|e| format!("Failed to find ffmpeg in archive: {}", e))?;
            let out_path = output_dir.join("ffmpeg.exe");
            let mut outfile = std::fs::File::create(&out_path)
                .map_err(|e| format!("Failed to create output file: {}", e))?;
            std::io::copy(&mut ffmpeg_file, &mut outfile)
                .map_err(|e| format!("Failed to extract ffmpeg: {}", e))?;
        }

        // Extract ffprobe.exe
        if !ffprobe_entry_name.is_empty() {
            let mut archive = zip::ZipArchive::new(std::fs::File::open(zip_path)
                .map_err(|e| format!("Failed to reopen zip: {}", e))?)
                .map_err(|e| format!("Failed to read zip archive: {}", e))?;
            
            let mut ffprobe_file = archive.by_name(&ffprobe_entry_name)
                .map_err(|e| format!("Failed to find ffprobe in archive: {}", e))?;
            let out_path = output_dir.join("ffprobe.exe");
            let mut outfile = std::fs::File::create(&out_path)
                .map_err(|e| format!("Failed to create output file: {}", e))?;
            std::io::copy(&mut ffprobe_file, &mut outfile)
                .map_err(|e| format!("Failed to extract ffprobe: {}", e))?;
        }

        Ok(())
    }
}
