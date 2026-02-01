import { Monitor, CheckCircle2 } from "lucide-react";
import type { GpuInfo as GpuInfoType } from "../types";

interface GpuInfoProps {
  info: GpuInfoType;
  selectedEncoder: string;
}

export default function GpuInfo({ info, selectedEncoder }: GpuInfoProps) {
  const getGpuBadgeClass = () => {
    switch (info.gpu_type) {
      case "Nvidia":
        return "gpu-badge-nvidia";
      case "Amd":
        return "gpu-badge-amd";
      case "Intel":
        return "gpu-badge-intel";
      default:
        return "gpu-badge-cpu";
    }
  };

  const isHardwareAccelerated = selectedEncoder && (
    selectedEncoder.includes("nvenc") ||
    selectedEncoder.includes("amf") ||
    selectedEncoder.includes("qsv")
  );

  // Get encoder type from the selected encoder
  const getEncoderType = () => {
    if (selectedEncoder.includes("nvenc")) return "NVIDIA NVENC";
    if (selectedEncoder.includes("amf")) return "AMD AMF";
    if (selectedEncoder.includes("qsv")) return "Intel QSV";
    if (selectedEncoder === "libx264") return "H.264 (CPU)";
    if (selectedEncoder === "libx265") return "H.265 (CPU)";
    return "CPU";
  };

  // Get only encoders matching the detected GPU type
  const getRelevantEncoders = () => {
    if (!info.available_encoders || info.available_encoders.length === 0) {
      return [];
    }
    
    // Filter encoders based on detected GPU
    return info.available_encoders.filter(encoder => {
      if (info.gpu_type === "Nvidia") {
        return encoder.name.includes("nvenc") || encoder.gpuType === "Nvidia";
      }
      if (info.gpu_type === "Amd") {
        return encoder.name.includes("amf") || encoder.gpuType === "Amd";
      }
      if (info.gpu_type === "Intel") {
        return encoder.name.includes("qsv") || encoder.gpuType === "Intel";
      }
      return false;
    });
  };

  const relevantEncoders = getRelevantEncoders();
  const hasGpuEncoders = relevantEncoders.length > 0;

  return (
    <div className="flex items-center gap-4">
      {/* GPU Info Card */}
      <div className="flex items-center gap-2 px-3 py-2 rounded-lg border" style={{ backgroundColor: '#1a1a1a', borderColor: '#333' }}>
        <Monitor size={20} style={{ color: '#888' }} />
        <div>
          <p className="text-xs" style={{ color: '#888' }}>GPU</p>
          <div className="text-sm font-medium">
            {info.detected ? (
              <div className="flex items-center gap-2">
                <span className={getGpuBadgeClass()}>
                  {info.gpu_type === "Nvidia" && "NVIDIA"}
                  {info.gpu_type === "Amd" && "AMD"}
                  {info.gpu_type === "Intel" && "Intel"}
                  {info.gpu_type === "Unknown" && "CPU"}
                  {info.gpu_type === "None" && "CPU"}
                </span>
                <span className="text-white truncate max-w-[150px] block" title={info.name}>
                  {info.name}
                </span>
              </div>
            ) : (
              <span style={{ color: '#888' }}>Not detected</span>
            )}
          </div>
        </div>
      </div>

      {/* Encoder Status Card */}
      <div className="flex items-center gap-2 px-3 py-2 rounded-lg border" style={{ backgroundColor: '#1a1a1a', borderColor: '#333' }}>
        <div className={`w-2 h-2 rounded-full ${isHardwareAccelerated ? 'bg-green-500' : 'bg-yellow-500'} ${isHardwareAccelerated ? 'animate-pulse' : ''}`} />
        <div>
          <p className="text-xs" style={{ color: '#888' }}>Encoder</p>
          <p className="text-sm font-medium">
            {isHardwareAccelerated ? (
              <span className="text-green-400 flex items-center gap-1">
                <CheckCircle2 size={12} />
                {getEncoderType()}
              </span>
            ) : (
              <span className="text-yellow-400">
                {getEncoderType()}
              </span>
            )}
          </p>
        </div>
      </div>

      {/* Available Encoders (if GPU detected) */}
      {hasGpuEncoders && (
        <div className="hidden lg:flex items-center gap-2 px-3 py-2 rounded-lg border" style={{ backgroundColor: 'rgba(26, 26, 26, 0.5)', borderColor: 'rgba(51, 51, 51, 0.5)' }}>
          <div>
            <p className="text-xs" style={{ color: '#888' }}>Available Encoders</p>
            <div className="flex items-center gap-2 mt-1">
              {relevantEncoders.map((encoder) => (
                <span
                  key={encoder.name}
                  className={`text-xs px-1.5 py-0.5 rounded ${
                    selectedEncoder === encoder.name
                      ? getGpuBadgeClass()
                      : "bg-gray-700 text-gray-400"
                  }`}
                  title={encoder.description}
                >
                  {encoder.name.includes("h264") || encoder.name.includes("264") ? "H.264" : 
                   encoder.name.includes("hevc") || encoder.name.includes("265") ? "HEVC" : 
                   encoder.codec}
                </span>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
