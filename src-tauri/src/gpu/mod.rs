use serde::{Deserialize, Serialize};
use tokio::process::Command;
use std::path::Path;
use regex::Regex;


// Windows creation flag to hide console window
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub detected: bool,
    pub gpu_type: GpuType,
    pub name: String,
    pub primary_adapter_id: Option<String>,
    pub adapters: Vec<GpuAdapter>,
    pub available_encoders: Vec<EncoderInfo>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GpuType {
    Nvidia,
    Intel,
    Amd,
    Unknown,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuAdapter {
    pub id: String,
    pub name: String,
    pub gpu_type: GpuType,
    pub is_virtual: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncoderInfo {
    pub name: String,
    pub description: String,
    pub codec: String,
    pub encoder_type: EncoderType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EncoderType {
    Cpu,
    GpuNvidia,
    GpuAmd,
    GpuIntel,
    Adobe,
}

pub struct GpuDetector;

impl GpuDetector {
    pub fn new() -> Self {
        Self
    }

    fn is_virtual_adapter(name: &str) -> bool {
        let name_lower = name.to_lowercase();
        let markers = [
            "virtual",
            "remote",
            "basic display",
            "microsoft basic",
            "miracast",
            "indirect display",
            "displaylink",
            "rdp",
            "vmware",
            "virtualbox",
            "parallels",
            "citrix",
            "xen",
            "dummy",
        ];

        markers.iter().any(|m| name_lower.contains(m))
    }

    fn classify_gpu_name(name: &str) -> GpuType {
        let name_upper = name.to_uppercase();
        if name_upper.contains("NVIDIA")
            || name_upper.contains("GEFORCE")
            || name_upper.contains("RTX")
            || name_upper.contains("GTX")
        {
            GpuType::Nvidia
        } else if name_upper.contains("AMD") || name_upper.contains("RADEON") {
            GpuType::Amd
        } else if name_upper.contains("INTEL")
            && (name_upper.contains("ARC")
                || name_upper.contains("UHD")
                || name_upper.contains("HD GRAPHICS")
                || name_upper.contains("IRIS"))
        {
            GpuType::Intel
        } else {
            GpuType::Unknown
        }
    }

    fn is_likely_integrated(name: &str) -> bool {
        let name_upper = name.to_uppercase();
        name_upper.contains("UHD")
            || name_upper.contains("HD GRAPHICS")
            || name_upper.contains("IRIS")
            || name_upper.contains("IRIS XE")
            || name_upper.contains("RADEON GRAPHICS")
            || name_upper.contains("INTEGRATED")
            || name_upper.contains("APU")
    }

    fn gpu_priority(name: &str, gpu_type: GpuType) -> i32 {
        let name_upper = name.to_uppercase();
        let mut score = match gpu_type {
            GpuType::Nvidia => 300,
            GpuType::Amd => 250,
            GpuType::Intel => 180,
            GpuType::Unknown => 100,
            GpuType::None => 0,
        };

        if name_upper.contains("RTX") {
            score += 60;
        } else if name_upper.contains("GTX") {
            score += 40;
        } else if name_upper.contains("ARC") {
            score += 30;
        } else if name_upper.contains("RX ") || name_upper.contains("RADEON RX") {
            score += 35;
        }

        if Self::is_likely_integrated(name) {
            score -= 55;
        }

        score
    }

    fn cleaned_non_empty_names(names: Vec<String>) -> Vec<String> {
        names
            .into_iter()
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .collect()
    }

    fn build_adapters(names: Vec<String>) -> Vec<GpuAdapter> {
        let cleaned_names = Self::cleaned_non_empty_names(names);

        cleaned_names
            .into_iter()
            .enumerate()
            .filter_map(|name| {
                let (index, name) = name;
                if Self::is_virtual_adapter(&name) {
                    return None;
                }

                Some(GpuAdapter {
                    id: format!("gpu-{}", index),
                    gpu_type: Self::classify_gpu_name(&name),
                    is_virtual: false,
                    name,
                })
            })
            .collect()
    }

    fn pick_primary_adapter(adapters: &[GpuAdapter]) -> Option<&GpuAdapter> {
        adapters.iter().max_by(|a, b| {
            let left = Self::gpu_priority(&a.name, a.gpu_type);
            let right = Self::gpu_priority(&b.name, b.gpu_type);
            left.cmp(&right)
        })
    }

    #[cfg(target_os = "windows")]
    async fn collect_gpu_names() -> Vec<String> {
        let mut wmic_names = Vec::new();
        let mut wmic_cmd = Command::new("wmic");
        wmic_cmd.args(["path", "win32_videocontroller", "get", "name", "/format:csv"]);
        wmic_cmd.creation_flags(CREATE_NO_WINDOW);

        if let Ok(output) = wmic_cmd.output().await {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with("Node") {
                    continue;
                }

                // CSV format: Node,DeviceID,Name
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() >= 3 {
                    wmic_names.push(parts[2].trim().to_string());
                }
            }
        }

        if !Self::cleaned_non_empty_names(wmic_names.clone()).is_empty() {
            return wmic_names;
        }

        let mut ps_names = Vec::new();
        let mut ps_cmd = Command::new("powershell");
        ps_cmd.args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_VideoController | Select-Object -ExpandProperty Name",
        ]);
        ps_cmd.creation_flags(CREATE_NO_WINDOW);

        if let Ok(output) = ps_cmd.output().await {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                ps_names.push(line.trim().to_string());
            }
        }

        ps_names
    }

    #[cfg(target_os = "linux")]
    async fn collect_gpu_names() -> Vec<String> {
        let mut cmd = Command::new("sh");
        cmd.args([
            "-lc",
            "lspci | grep -Ei 'vga|3d|display' | sed -E 's/^.*: //'",
        ]);

        match cmd.output().await {
            Ok(output) => String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|line| line.trim().to_string())
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    #[cfg(target_os = "macos")]
    async fn collect_gpu_names() -> Vec<String> {
        let mut cmd = Command::new("system_profiler");
        cmd.args(["SPDisplaysDataType", "-json"]);

        let mut names = Vec::new();
        if let Ok(output) = cmd.output().await {
            if output.status.success() {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                    if let Some(items) = json
                        .get("SPDisplaysDataType")
                        .and_then(|v| v.as_array())
                    {
                        for item in items {
                            if let Some(name) = item.get("sppci_model").and_then(|v| v.as_str()) {
                                names.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }

        names
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    async fn collect_gpu_names() -> Vec<String> {
        Vec::new()
    }

    /// Detect GPU information and available encoders
    pub async fn detect() -> Result<GpuInfo, Box<dyn std::error::Error>> {
        Self::detect_with_ffmpeg(None).await
    }

    /// Detect GPU information with specific ffmpeg path
    pub async fn detect_with_ffmpeg(ffmpeg_path: Option<&str>) -> Result<GpuInfo, Box<dyn std::error::Error>> {
        let names = Self::collect_gpu_names().await;
        let adapters = Self::build_adapters(names);
        let primary = Self::pick_primary_adapter(&adapters);
        let gpu_name = primary.map(|a| a.name.clone()).unwrap_or_default();
        let primary_adapter_id = primary.map(|a| a.id.clone());
        let gpu_type = primary.map(|a| a.gpu_type).unwrap_or(GpuType::None);

        // Get available encoders by running ffmpeg -encoders
        let available_encoders = Self::get_available_encoders(ffmpeg_path).await?;

        Ok(GpuInfo {
            detected: !matches!(gpu_type, GpuType::None),
            gpu_type,
            name: gpu_name,
            primary_adapter_id,
            adapters,
            available_encoders,
        })
    }

    /// Get available encoders by running `ffmpeg -encoders`
    pub async fn get_available_encoders(ffmpeg_path: Option<&str>) -> Result<Vec<EncoderInfo>, Box<dyn std::error::Error>> {
        println!("  get_available_encoders called with path: {:?}", ffmpeg_path);

        // If a full path is provided, verify it exists first
        if let Some(path_str) = ffmpeg_path {
            let path = Path::new(path_str);
            if path.is_absolute() && !path.exists() {
                println!("  ERROR: FFmpeg path does not exist: {}", path_str);
                return Err(format!("FFmpeg not found at: {}", path_str).into());
            }
            if path.is_absolute() {
                println!("  FFmpeg path exists: {}", path_str);
            }
        }

        let ffmpeg = ffmpeg_path.unwrap_or("ffmpeg");
        println!("  Using FFmpeg: {}", ffmpeg);
        let mut cmd = Command::new(ffmpeg);
        cmd.arg("-encoders");

        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

        println!("  Executing: {} -encoders", ffmpeg);

        let output = match cmd.output().await {
            Ok(output) => {
                println!("  ✓ Command succeeded!");
                println!("  Status: {}, stdout: {} bytes, stderr: {} bytes",
                    output.status, output.stdout.len(), output.stderr.len());

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    println!("  stderr: {}", stderr);
                }
                output
            }
            Err(e) => {
                println!("  ✗ Command failed: {:?}. Falling back to default encoders.", e);
                return Ok(Self::get_default_encoders());
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut encoders = Vec::new();

        // Parse ffmpeg -encoders output
        // Format: V..... libx264              H.264 / AVC / MPEG-4 AVC / MPEG-4 part 10 (codec h264)
        let encoder_regex = Regex::new(r"^\s*([VASFXD\.]{6})\s+(\S+)\s+(.+)$").unwrap();
        let codec_regex = Regex::new(r"\(codec\s+(\w+)\)").unwrap();

        for line in stdout.lines() {
            if let Some(captures) = encoder_regex.captures(line) {
                let flags = &captures[1];
                let name = captures[2].to_string();
                let mut description = captures[3].to_string();
                
                // Extract codec from description if present
                let codec = if let Some(codec_caps) = codec_regex.captures(&description) {
                    codec_caps[1].to_string()
                } else {
                    // Try to infer codec from encoder name
                    Self::infer_codec(&name)
                };

                // Remove codec info from description for cleaner display
                if let Some(pos) = description.find(" (codec") {
                    description = description[..pos].to_string();
                }

                // Determine encoder type and filter relevant encoders
                if let Some(encoder_type) = Self::classify_encoder(&name) {
                    // Only include video encoders (V flag)
                    if flags.contains('V') {
                        encoders.push(EncoderInfo {
                            name,
                            description,
                            codec,
                            encoder_type,
                        });
                    }
                }
            }
        }

        // Sort encoders: CPU first, then GPU by type, then Adobe codecs
        encoders.sort_by(|a, b| {
            let type_order = |e: &EncoderInfo| match e.encoder_type {
                EncoderType::Cpu => 0,
                EncoderType::GpuNvidia => 1,
                EncoderType::GpuAmd => 2,
                EncoderType::GpuIntel => 3,
                EncoderType::Adobe => 4,
            };
            type_order(a).cmp(&type_order(b))
        });

        // If ffmpeg command failed or no encoders found, return default list
        if encoders.is_empty() {
            encoders = Self::get_default_encoders();
        }

        Ok(encoders)
    }

    /// Classify encoder by type based on name
    fn classify_encoder(name: &str) -> Option<EncoderType> {
        let name_lower = name.to_lowercase();
        
        // GPU encoders
        if name_lower.contains("nvenc") {
            return Some(EncoderType::GpuNvidia);
        }
        if name_lower.contains("amf") || name_lower.contains("vaapi") && name_lower.contains("h264") {
            return Some(EncoderType::GpuAmd);
        }
        if name_lower.contains("qsv") || name_lower.contains("mediacodec") {
            return Some(EncoderType::GpuIntel);
        }
        
        // Adobe/Professional encoders
        if name_lower.contains("prores") || name_lower.contains("dnxhd") || name_lower.contains("cfhd") || name_lower.contains("cineform") {
            return Some(EncoderType::Adobe);
        }
        
        // CPU encoders - common video codecs
        if name_lower.contains("libx264") 
            || name_lower.contains("libx265") 
            || name_lower.contains("libxvid")
            || name_lower.contains("libvpx")
            || name_lower.contains("libaom")
            || name_lower.contains("libsvtav1")
            || name_lower.contains("mpeg")
            || name_lower.contains("wmv")
            || name_lower.contains("flv")
            || name_lower.contains("h263")
            || name_lower.contains("huffyuv")
            || name_lower.contains("ffv")
            || name_lower.contains("rawvideo")
            || name_lower.contains("libtheora") {
            return Some(EncoderType::Cpu);
        }
        
        None
    }

    /// Infer codec from encoder name
    fn infer_codec(name: &str) -> String {
        let name_lower = name.to_lowercase();
        
        if name_lower.contains("264") || name_lower.contains("h264") {
            "h264".to_string()
        } else if name_lower.contains("265") || name_lower.contains("hevc") || name_lower.contains("x265") {
            "hevc".to_string()
        } else if name_lower.contains("vp8") {
            "vp8".to_string()
        } else if name_lower.contains("vp9") {
            "vp9".to_string()
        } else if name_lower.contains("av1") {
            "av1".to_string()
        } else if name_lower.contains("mpeg4") || name_lower.contains("xvid") {
            "mpeg4".to_string()
        } else if name_lower.contains("mpeg2") {
            "mpeg2video".to_string()
        } else if name_lower.contains("mpeg1") {
            "mpeg1video".to_string()
        } else if name_lower.contains("wmv") {
            "wmv2".to_string()
        } else if name_lower.contains("flv") {
            "flv1".to_string()
        } else if name_lower.contains("prores") {
            "prores".to_string()
        } else if name_lower.contains("dnxhd") || name_lower.contains("dnxhr") {
            "dnxhd".to_string()
        } else if name_lower.contains("cineform") || name_lower.contains("cfhd") {
            "cineform".to_string()
        } else if name_lower.contains("theora") {
            "theora".to_string()
        } else {
            "unknown".to_string()
        }
    }

    /// Get default encoders when ffmpeg is not available
    fn get_default_encoders() -> Vec<EncoderInfo> {
        vec![
            EncoderInfo {
                name: "libx264".to_string(),
                description: "H.264 / AVC / MPEG-4 AVC / MPEG-4 part 10".to_string(),
                codec: "h264".to_string(),
                encoder_type: EncoderType::Cpu,
            },
            EncoderInfo {
                name: "libx265".to_string(),
                description: "H.265 / HEVC (High Efficiency Video Coding)".to_string(),
                codec: "hevc".to_string(),
                encoder_type: EncoderType::Cpu,
            },
            EncoderInfo {
                name: "h264_nvenc".to_string(),
                description: "NVIDIA NVENC H.264 encoder".to_string(),
                codec: "h264".to_string(),
                encoder_type: EncoderType::GpuNvidia,
            },
            EncoderInfo {
                name: "hevc_nvenc".to_string(),
                description: "NVIDIA NVENC HEVC encoder".to_string(),
                codec: "hevc".to_string(),
                encoder_type: EncoderType::GpuNvidia,
            },
            EncoderInfo {
                name: "h264_amf".to_string(),
                description: "AMD AMF H.264 Encoder".to_string(),
                codec: "h264".to_string(),
                encoder_type: EncoderType::GpuAmd,
            },
            EncoderInfo {
                name: "hevc_amf".to_string(),
                description: "AMD AMF HEVC encoder".to_string(),
                codec: "hevc".to_string(),
                encoder_type: EncoderType::GpuAmd,
            },
            EncoderInfo {
                name: "h264_qsv".to_string(),
                description: "H.264 / AVC / MPEG-4 AVC / MPEG-4 part 10 (Intel Quick Sync Video acceleration)".to_string(),
                codec: "h264".to_string(),
                encoder_type: EncoderType::GpuIntel,
            },
            EncoderInfo {
                name: "hevc_qsv".to_string(),
                description: "HEVC (Intel Quick Sync Video acceleration)".to_string(),
                codec: "hevc".to_string(),
                encoder_type: EncoderType::GpuIntel,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_discrete_gpu_above_integrated() {
        let names = vec![
            "Intel(R) UHD Graphics".to_string(),
            "NVIDIA GeForce GTX 1660 Ti".to_string(),
        ];

        let adapters = GpuDetector::build_adapters(names);
        let primary = GpuDetector::pick_primary_adapter(&adapters);
        assert_eq!(primary.map(|a| a.gpu_type), Some(GpuType::Nvidia));
    }

    #[test]
    fn filters_virtual_adapters() {
        let names = vec![
            "Microsoft Basic Display Adapter".to_string(),
            "NVIDIA GeForce RTX 4060".to_string(),
        ];

        let adapters = GpuDetector::build_adapters(names);
        assert_eq!(adapters.len(), 1);
        assert_eq!(adapters[0].gpu_type, GpuType::Nvidia);
    }

    #[test]
    fn preserves_identical_model_entries() {
        let names = vec![
            "NVIDIA GeForce RTX 4090".to_string(),
            "NVIDIA GeForce RTX 4090".to_string(),
        ];

        let adapters = GpuDetector::build_adapters(names);
        assert_eq!(adapters.len(), 2);
        assert_eq!(adapters[0].id, "gpu-0");
        assert_eq!(adapters[1].id, "gpu-1");
    }
}

/// Get encoder display name based on encoder info
pub fn get_encoder_display_name(encoder: &EncoderInfo) -> String {
    match encoder.encoder_type {
        EncoderType::Cpu => {
            format!("{} (CPU) - {}", encoder.name, encoder.description)
        }
        EncoderType::GpuNvidia => {
            format!("{} (NVIDIA GPU) - {}", encoder.name, encoder.description)
        }
        EncoderType::GpuAmd => {
            format!("{} (AMD GPU) - {}", encoder.name, encoder.description)
        }
        EncoderType::GpuIntel => {
            format!("{} (Intel GPU) - {}", encoder.name, encoder.description)
        }
        EncoderType::Adobe => {
            format!("{} (Professional) - {}", encoder.name, encoder.description)
        }
    }
}

/// Check if specific encoder is available
pub async fn is_encoder_available(ffmpeg_path: &str, encoder_name: &str) -> bool {
    match GpuDetector::get_available_encoders(Some(ffmpeg_path)).await {
        Ok(encoders) => encoders.iter().any(|e| e.name == encoder_name),
        Err(_) => false,
    }
}
