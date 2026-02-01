export type InputFormat = "mkv" | "mp4" | "avi" | "mov" | "wmv" | "flv" | "webm";

export type OutputFormat = 
  | "mp4" 
  | "mkv" 
  | "mp3" 
  | "wav" 
  | "aac" 
  | "flac" 
  | "m4a" 
  | "avi" 
  | "mov" 
  | "prores" 
  | "dnxhd";

export type AdobePreset = 
  | "prores_422" 
  | "prores_422_hq" 
  | "prores_4444" 
  | "prores_4444_xq"
  | "dnxhd_220" 
  | "dnxhd_220x"
  | "cineform_yuv"
  | "cineform_rgb";

export interface GpuInfo {
  detected: boolean;
  gpu_type: "Nvidia" | "Intel" | "Amd" | "Unknown" | "None";
  name: string;
  available_encoders: EncoderInfo[];
}

export interface EncoderInfo {
  name: string;
  description: string;
  codec: string;
  gpuType: "Nvidia" | "Intel" | "Amd" | "CPU";
}

export interface AppSettings {
  ffmpegPath: string;
  outputDir: string;
  encoder: string;
  preset: string;
  inputFormat: InputFormat;
  outputFormat: OutputFormat;
  adobeCompatible: boolean;
  adobePreset: AdobePreset;
  autoDownloadFfmpeg: boolean;
}

export interface ConversionTask {
  id: string;
  inputFile: string;
  outputFile: string;
  inputFormat: InputFormat;
  outputFormat: OutputFormat;
  status: "pending" | "converting" | "completed" | "failed" | "cancelled";
  progress: number;
  logs: string[];
}

export interface FfmpegDownloadStatus {
  downloading: boolean;
  progress: number;
  status: "idle" | "downloading" | "extracting" | "completed" | "failed";
  message: string;
}

export interface FfmpegStatus {
  available: boolean;
  path?: string;
  version?: string;
  source?: string;
}
