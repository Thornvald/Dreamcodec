import { Folder, FileCode, Settings2, ChevronDown, Sparkles, Clapperboard, Music, Film, CheckCircle, AlertCircle } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import type { AppSettings, GpuInfo, InputFormat, OutputFormat, AdobePreset, FfmpegStatus } from "../types";

interface SettingsProps {
  settings: AppSettings;
  onSettingsChange: (settings: AppSettings) => void;
  gpuInfo: GpuInfo | null;
  ffmpegStatus: FfmpegStatus | null;
}

const presets = ["ultrafast", "superfast", "veryfast", "faster", "fast", "medium", "slow", "slower", "veryslow"];

const inputFormats: { value: InputFormat; label: string; icon: string }[] = [
  { value: "mkv", label: "MKV", icon: "video" },
  { value: "mp4", label: "MP4", icon: "video" },
  { value: "avi", label: "AVI", icon: "video" },
  { value: "mov", label: "MOV", icon: "video" },
  { value: "wmv", label: "WMV", icon: "video" },
  { value: "flv", label: "FLV", icon: "video" },
  { value: "webm", label: "WEBM", icon: "video" },
];

const outputFormats: { value: OutputFormat; label: string; icon: string; type: "video" | "audio" | "pro" }[] = [
  { value: "mp4", label: "MP4", icon: "film", type: "video" },
  { value: "mkv", label: "MKV", icon: "film", type: "video" },
  { value: "avi", label: "AVI", icon: "film", type: "video" },
  { value: "mov", label: "MOV", icon: "film", type: "video" },
  { value: "mp3", label: "MP3", icon: "music", type: "audio" },
  { value: "wav", label: "WAV", icon: "music", type: "audio" },
  { value: "aac", label: "AAC", icon: "music", type: "audio" },
  { value: "flac", label: "FLAC", icon: "music", type: "audio" },
  { value: "m4a", label: "M4A", icon: "music", type: "audio" },
  { value: "prores", label: "ProRes", icon: "clapperboard", type: "pro" },
  { value: "dnxhd", label: "DNxHD", icon: "clapperboard", type: "pro" },
];

const adobePresets: { value: AdobePreset; label: string; description: string }[] = [
  { value: "prores_422", label: "ProRes 422", description: "Standard quality, good for editing" },
  { value: "prores_422_hq", label: "ProRes 422 HQ", description: "High quality, larger files" },
  { value: "prores_4444", label: "ProRes 4444", description: "Maximum quality with alpha" },
  { value: "prores_4444_xq", label: "ProRes 4444 XQ", description: "Extended quality with alpha" },
  { value: "dnxhd_220", label: "DNxHD 220", description: "Broadcast quality 1080p" },
  { value: "dnxhd_220x", label: "DNxHD 220x", description: "High bitrate broadcast quality" },
  { value: "cineform_yuv", label: "CineForm YUV", description: "After Effects compatible" },
  { value: "cineform_rgb", label: "CineForm RGB", description: "Maximum color fidelity" },
];

// CPU encoders for reference
const cpuEncoders = [
  { name: "libx264", description: "H.264 (CPU)", codec: "h264", gpuType: "CPU" as const },
  { name: "libx265", description: "H.265/HEVC (CPU)", codec: "hevc", gpuType: "CPU" as const },
];

export default function Settings({
  settings,
  onSettingsChange,
  gpuInfo,
  ffmpegStatus,
}: SettingsProps) {
  const handleSelectOutputDir = async () => {
    const selected = await open({
      directory: true,
    });

    if (selected && typeof selected === "string") {
      onSettingsChange({ ...settings, outputDir: selected });
    }
  };

  // Get available encoders based on detected GPU
  const getAvailableEncoders = () => {
    if (!gpuInfo || gpuInfo.available_encoders.length === 0) {
      return cpuEncoders;
    }
    return gpuInfo.available_encoders;
  };

  const getEncoderBadgeClass = (encoderName: string) => {
    if (encoderName.includes("nvenc")) return "gpu-badge-nvidia";
    if (encoderName.includes("amf")) return "gpu-badge-amd";
    if (encoderName.includes("qsv")) return "gpu-badge-intel";
    return "gpu-badge-cpu";
  };

  const getSourceLabel = (source?: string) => {
    switch (source) {
      case "bundled": return "Bundled with app";
      case "path": return "System PATH";
      case "common": return "Common location";
      case "winget": return "WinGet package";
      case "downloaded": return "Downloaded";
      default: return "Auto-detected";
    }
  };

  return (
    <div className="space-y-5">
      <div className="flex items-center gap-2 mb-4">
        <Settings2 size={20} className="text-secondary" />
        <h2 className="text-lg font-semibold">Settings</h2>
      </div>

      {/* FFmpeg Status - Auto-detected */}
      <div className="space-y-2">
        <label className="text-sm font-medium text-secondary flex items-center gap-2">
          <FileCode size={16} />
          FFmpeg Status
        </label>
        <div className="p-3 rounded-lg border border-border-primary bg-tertiary">
          {ffmpegStatus?.available ? (
            <div className="flex items-center gap-2">
              <CheckCircle size={18} className="text-green-500" />
              <div className="flex-1 min-w-0">
                <p className="text-sm font-medium text-primary">FFmpeg detected</p>
                <p className="text-xs text-muted truncate">
                  {getSourceLabel(ffmpegStatus.source)}
                </p>
              </div>
            </div>
          ) : (
            <div className="flex items-center gap-2">
              <AlertCircle size={18} className="text-red-500" />
              <div className="flex-1">
                <p className="text-sm font-medium text-primary">FFmpeg not found</p>
                <p className="text-xs text-muted">
                  Will be installed with setup
                </p>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Output Directory */}
      <div className="space-y-2">
        <label className="text-sm font-medium text-secondary flex items-center gap-2">
          <Folder size={16} />
          Output Directory
        </label>
        <div className="flex gap-2">
          <input
            type="text"
            value={settings.outputDir}
            readOnly
            placeholder="Select output folder..."
            className="input text-sm"
          />
          <button
            onClick={handleSelectOutputDir}
            className="btn btn-secondary whitespace-nowrap text-sm"
          >
            Browse
          </button>
        </div>
      </div>

      {/* Format Selection */}
      <div className="space-y-3 pt-2 border-t border-border-primary">
        <h3 className="text-sm font-medium text-secondary flex items-center gap-2">
          <Sparkles size={16} className="text-secondary" />
          Format Selection
        </h3>
        
        {/* Input Format */}
        <div className="space-y-2">
          <label className="text-xs font-medium text-muted">Input Format</label>
          <div className="relative select-glow">
            <select
              value={settings.inputFormat}
              onChange={(e) => onSettingsChange({ ...settings, inputFormat: e.target.value as InputFormat })}
              className="input text-sm appearance-none cursor-pointer"
            >
              {inputFormats.map((format) => (
                <option key={format.value} value={format.value}>
                  {format.label}
                </option>
              ))}
            </select>
            <ChevronDown
              size={16}
              className="absolute right-3 top-1/2 transform -translate-y-1/2 text-muted pointer-events-none"
            />
          </div>
        </div>

        {/* Output Format */}
        <div className="space-y-2">
          <label className="text-xs font-medium text-muted">Output Format</label>
          <div className="relative select-glow">
            <select
              value={settings.outputFormat}
              onChange={(e) => onSettingsChange({ 
                ...settings, 
                outputFormat: e.target.value as OutputFormat,
                adobeCompatible: ["mp4", "mov", "prores", "dnxhd"].includes(e.target.value) 
                  ? settings.adobeCompatible 
                  : false
              })}
              className="input text-sm appearance-none cursor-pointer"
            >
              <optgroup label="Video Formats">
                {outputFormats.filter(f => f.type === "video").map((format) => (
                  <option key={format.value} value={format.value}>
                    {format.label}
                  </option>
                ))}
              </optgroup>
              <optgroup label="Audio Only">
                {outputFormats.filter(f => f.type === "audio").map((format) => (
                  <option key={format.value} value={format.value}>
                    {format.label}
                  </option>
                ))}
              </optgroup>
              <optgroup label="Professional (Adobe Compatible)">
                {outputFormats.filter(f => f.type === "pro").map((format) => (
                  <option key={format.value} value={format.value}>
                    {format.label}
                  </option>
                ))}
              </optgroup>
            </select>
            <ChevronDown
              size={16}
              className="absolute right-3 top-1/2 transform -translate-y-1/2 text-muted pointer-events-none"
            />
          </div>
          
          {/* Format indicator badges */}
          <div className="flex flex-wrap gap-2 mt-2">
            {outputFormats.find(f => f.value === settings.outputFormat)?.type === "pro" && (
              <span className="adobe-badge">
                <Clapperboard size={12} />
                Pro Format
              </span>
            )}
            {outputFormats.find(f => f.value === settings.outputFormat)?.type === "audio" && (
              <span className="format-icon format-audio">
                <Music size={10} />
              </span>
            )}
            {outputFormats.find(f => f.value === settings.outputFormat)?.type === "video" && (
              <span className="format-icon format-video">
                <Film size={10} />
              </span>
            )}
          </div>
        </div>
      </div>

      {/* Adobe After Effects Compatible */}
      {["mp4", "mov", "prores", "dnxhd"].includes(settings.outputFormat) && (
        <div className="space-y-3 pt-2 border-t border-border-primary">
          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="adobeCompatible"
              checked={settings.adobeCompatible}
              onChange={(e) => onSettingsChange({ ...settings, adobeCompatible: e.target.checked })}
              className="checkbox-glow w-4 h-4"
            />
            <label htmlFor="adobeCompatible" className="text-sm font-medium text-secondary flex items-center gap-2">
              <Clapperboard size={16} className="text-secondary" />
              Adobe / After Effects Compatible
            </label>
          </div>

          {settings.adobeCompatible && (
            <div className="space-y-2 pl-6 animate-fade-in">
              <label className="text-xs font-medium text-muted">Professional Preset</label>
              <div className="relative select-glow">
                <select
                  value={settings.adobePreset}
                  onChange={(e) => onSettingsChange({ ...settings, adobePreset: e.target.value as AdobePreset })}
                  className="input text-sm appearance-none cursor-pointer"
                >
                  {adobePresets.map((preset) => (
                    <option key={preset.value} value={preset.value}>
                      {preset.label} - {preset.description}
                    </option>
                  ))}
                </select>
                <ChevronDown
                  size={16}
                  className="absolute right-3 top-1/2 transform -translate-y-1/2 text-muted pointer-events-none"
                />
              </div>
              
              {/* Selected preset info */}
              <div className="p-3 rounded-lg mt-2 bg-quaternary border border-border-primary">
                <p className="text-xs text-secondary">
                  {adobePresets.find(p => p.value === settings.adobePreset)?.description}
                </p>
              </div>
            </div>
          )}
        </div>
      )}

      {/* Encoder Selection */}
      {!settings.adobeCompatible && (
        <div className="space-y-2 pt-2 border-t border-border-primary">
          <label className="text-sm font-medium text-secondary">Video Encoder</label>
          <div className="relative select-glow">
            <select
              value={settings.encoder}
              onChange={(e) => onSettingsChange({ ...settings, encoder: e.target.value })}
              className="input text-sm appearance-none cursor-pointer"
            >
              <option value="">Select encoder...</option>
              {getAvailableEncoders().map((encoder) => (
                <option key={encoder.name} value={encoder.name}>
                  {encoder.description}
                </option>
              ))}
            </select>
            <ChevronDown
              size={16}
              className="absolute right-3 top-1/2 transform -translate-y-1/2 text-muted pointer-events-none"
            />
          </div>
          
          {/* Encoder badges */}
          {settings.encoder && (
            <div className="flex flex-wrap gap-2 mt-2">
              <span className={getEncoderBadgeClass(settings.encoder)}>
                {settings.encoder.includes("nvenc") && "NVIDIA"}
                {settings.encoder.includes("amf") && "AMD"}
                {settings.encoder.includes("qsv") && "Intel"}
                {(settings.encoder === "libx264" || settings.encoder === "libx265") && "CPU"}
              </span>
            </div>
          )}
          
          <p className="text-xs text-muted">
            GPU encoders provide faster conversion speeds
          </p>
        </div>
      )}

      {/* Preset Selection */}
      {!settings.adobeCompatible && (
        <div className="space-y-2">
          <label className="text-sm font-medium text-secondary">Preset</label>
          <div className="relative select-glow">
            <select
              value={settings.preset}
              onChange={(e) => onSettingsChange({ ...settings, preset: e.target.value })}
              className="input text-sm appearance-none cursor-pointer"
            >
              {presets.map((preset) => (
                <option key={preset} value={preset}>
                  {preset.charAt(0).toUpperCase() + preset.slice(1)}
                </option>
              ))}
            </select>
            <ChevronDown
              size={16}
              className="absolute right-3 top-1/2 transform -translate-y-1/2 text-muted pointer-events-none"
            />
          </div>
          <p className="text-xs text-muted">
            Faster presets = larger file size, slower = better compression
          </p>
        </div>
      )}

      {/* Info Box */}
      <div className="p-4 rounded-lg border border-border-primary bg-tertiary">
        <h4 className="font-medium text-sm mb-2 text-secondary">Encoder Info</h4>
        <ul className="text-xs text-muted space-y-1">
          <li className="flex items-center gap-2">
            <span className="gpu-badge-nvidia">NVIDIA</span>
            <span>NVENC - GeForce GTX 600+</span>
          </li>
          <li className="flex items-center gap-2">
            <span className="gpu-badge-amd">AMD</span>
            <span>AMF - Radeon HD 7000+</span>
          </li>
          <li className="flex items-center gap-2">
            <span className="gpu-badge-intel">Intel</span>
            <span>QSV - 4th Gen Core+</span>
          </li>
          <li className="flex items-center gap-2">
            <span className="gpu-badge-cpu">CPU</span>
            <span>Software - Works on all systems</span>
          </li>
        </ul>
      </div>
    </div>
  );
}
