import { useState, useEffect, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import StarBackground from "./components/StarBackground";
import "remixicon/fonts/remixicon.css";

type EncoderType = "Cpu" | "GpuNvidia" | "GpuAmd" | "GpuIntel" | "Adobe";
type GpuType = "Nvidia" | "Intel" | "Amd" | "Unknown" | "None";

interface Encoder {
  name: string;
  description: string;
  codec: string;
  encoder_type: EncoderType;
}

interface GpuAdapter {
  id: string;
  name: string;
  gpu_type: GpuType;
  is_virtual: boolean;
}

interface GpuInfo {
  detected: boolean;
  gpu_type: GpuType;
  name: string;
  primary_adapter_id?: string | null;
  adapters: GpuAdapter[];
  available_encoders: Encoder[];
}

interface GpuPreferenceOption {
  value: string;
  label: string;
}

interface FfmpegStatus {
  available: boolean;
  path?: string;
  version?: string;
  source?: string;
}

interface CpuInfo {
  name: string;
  logical_cores: number;
}

interface QueueFile {
  path: string;
  name: string;
}

type ConversionStatus = "pending" | "converting" | "completed" | "failed" | "cancelled";

interface ConversionParams {
  encoder: string;
  gpuIndex: number | undefined;
  cpuThreads: number | undefined;
  preset: string;
}

interface ConversionItem {
  id: string;
  inputFile: string;
  outputFile: string;
  status: ConversionStatus;
  progress: number;
  failureMessage?: string | null;
  params?: ConversionParams;
}

interface ConversionProgress {
  status: unknown;
  percentage: number;
  log?: string[];
  error_message?: string | null;
}

const SUPPORTED_INPUT_EXTENSIONS = new Set(["mkv", "mp4", "avi", "mov", "wmv", "flv", "webm"]);
const PREFERENCE_CACHE_KEY = "dreamcodec.preferences.v1";
const HARDWARE_CACHE_KEY = "dreamcodec.hardware.v1";
const HARDWARE_CACHE_VERSION = 1;

interface PreferenceCache {
  encoder: string;
  gpuPreference: string;
  cpuLimit: number;
  maxConcurrent: number;
}

interface HardwareCache {
  version: number;
  savedAt: number;
  cpuInfo: CpuInfo | null;
  gpuInfo: GpuInfo;
}

const readPreferenceCache = (): PreferenceCache => {
  const defaults: PreferenceCache = { encoder: "", gpuPreference: "auto", cpuLimit: 100, maxConcurrent: 3 };
  if (typeof window === "undefined") return defaults;

  try {
    const raw = window.localStorage.getItem(PREFERENCE_CACHE_KEY);
    if (!raw) return defaults;

    const parsed = JSON.parse(raw) as Partial<PreferenceCache>;
    return {
      encoder: typeof parsed.encoder === "string" ? parsed.encoder : "",
      gpuPreference:
        typeof parsed.gpuPreference === "string" && parsed.gpuPreference.length > 0
          ? parsed.gpuPreference
          : "auto",
      cpuLimit: typeof parsed.cpuLimit === "number" && [25, 50, 75, 100].includes(parsed.cpuLimit)
        ? parsed.cpuLimit
        : 100,
      maxConcurrent: typeof parsed.maxConcurrent === "number" && parsed.maxConcurrent >= 1 && parsed.maxConcurrent <= 5
        ? parsed.maxConcurrent
        : 3,
    };
  } catch {
    return defaults;
  }
};

const writePreferenceCache = (update: Partial<PreferenceCache>) => {
  if (typeof window === "undefined") return;

  const current = readPreferenceCache();
  const next: PreferenceCache = {
    encoder: update.encoder ?? current.encoder,
    gpuPreference: update.gpuPreference ?? current.gpuPreference,
    cpuLimit: update.cpuLimit ?? current.cpuLimit,
    maxConcurrent: update.maxConcurrent ?? current.maxConcurrent,
  };

  try {
    window.localStorage.setItem(PREFERENCE_CACHE_KEY, JSON.stringify(next));
  } catch {
    // Ignore storage write failures.
  }
};

const isGpuInfoLike = (value: unknown): value is GpuInfo => {
  if (!value || typeof value !== "object") return false;
  const maybe = value as Partial<GpuInfo>;
  return Array.isArray(maybe.adapters) && Array.isArray(maybe.available_encoders);
};

const readHardwareCache = (): HardwareCache | null => {
  if (typeof window === "undefined") return null;

  try {
    const raw = window.localStorage.getItem(HARDWARE_CACHE_KEY);
    if (!raw) return null;

    const parsed = JSON.parse(raw) as Partial<HardwareCache>;
    if (parsed.version !== HARDWARE_CACHE_VERSION) return null;
    if (typeof parsed.savedAt !== "number") return null;
    if (!isGpuInfoLike(parsed.gpuInfo)) return null;

    const cpuInfo =
      parsed.cpuInfo &&
      typeof parsed.cpuInfo === "object" &&
      typeof (parsed.cpuInfo as Partial<CpuInfo>).name === "string" &&
      typeof (parsed.cpuInfo as Partial<CpuInfo>).logical_cores === "number"
        ? (parsed.cpuInfo as CpuInfo)
        : null;

    return {
      version: HARDWARE_CACHE_VERSION,
      savedAt: parsed.savedAt,
      cpuInfo,
      gpuInfo: parsed.gpuInfo,
    };
  } catch {
    return null;
  }
};

const writeHardwareCache = (cpuInfo: CpuInfo | null, gpuInfo: GpuInfo) => {
  if (typeof window === "undefined") return;

  const payload: HardwareCache = {
    version: HARDWARE_CACHE_VERSION,
    savedAt: Date.now(),
    cpuInfo,
    gpuInfo,
  };

  try {
    window.localStorage.setItem(HARDWARE_CACHE_KEY, JSON.stringify(payload));
  } catch {
    // Ignore storage write failures.
  }
};

export default function App() {
  const [activeTab, setActiveTab] = useState("queue");
  const [outputDir, setOutputDir] = useState("");
  const [encoder, setEncoder] = useState(() => readPreferenceCache().encoder);
  const [preset, setPreset] = useState("fast");
  const [outputFormat, setOutputFormat] = useState("mp4");
  const [queue, setQueue] = useState<QueueFile[]>([]);
  const [encoders, setEncoders] = useState<Encoder[]>([]);
  const [allEncoders, setAllEncoders] = useState<Encoder[]>([]);
  const [gpuInfo, setGpuInfo] = useState<GpuInfo | null>(null);
  const [gpuPreference, setGpuPreference] = useState(() => readPreferenceCache().gpuPreference);
  const [cpuLimit, setCpuLimit] = useState(() => readPreferenceCache().cpuLimit);
  const [maxConcurrent, setMaxConcurrent] = useState(() => readPreferenceCache().maxConcurrent);
  const [cpuInfo, setCpuInfo] = useState<CpuInfo | null>(null);
  const [gpuName, setGpuName] = useState("");
  const [conversions, setConversions] = useState<ConversionItem[]>([]);
  const [logs, setLogs] = useState<string[]>([]);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [isHardwareInitializing, setIsHardwareInitializing] = useState(true);
  const [hardwareInitError, setHardwareInitError] = useState<string | null>(null);
  const [isDragOverlayVisible, setIsDragOverlayVisible] = useState(false);
  const [draggedFileCount, setDraggedFileCount] = useState(0);
  const conversionsRef = useRef<ConversionItem[]>([]);
  const pollerRef = useRef<number | null>(null);
  const [panicInfo, setPanicInfo] = useState<{ payload: string, location: string } | null>(null);

  useEffect(() => {
    conversionsRef.current = conversions;
  }, [conversions]);

  // Auto-start next pending conversion when a slot opens up
  useEffect(() => {
    const activeCount = conversions.filter(c => c.status === "converting").length;
    const hasPending = conversions.some(c => c.status === "pending" && c.params);
    if (hasPending && activeCount < maxConcurrent) {
      void startNextPending();
    }
  }, [conversions, maxConcurrent]);

  useEffect(() => {
    writePreferenceCache({ encoder });
  }, [encoder]);

  useEffect(() => {
    writePreferenceCache({ gpuPreference });
  }, [gpuPreference]);

  useEffect(() => {
    writePreferenceCache({ cpuLimit });
  }, [cpuLimit]);

  useEffect(() => {
    writePreferenceCache({ maxConcurrent });
  }, [maxConcurrent]);

  const addLog = (level: "info" | "warn" | "error", message: string) => {
    invoke("log_message", { level, message });
  };

  useEffect(() => {
    const interval = setInterval(() => {
      invoke<string>("get_log_file_content")
        .then(content => {
          setLogs(content.split("\n"));
        })
        .catch(console.error);
    }, 2000);

    const unlisten = listen("panic", (event) => {
      setPanicInfo(event.payload as { payload: string, location: string });
    });

    return () => {
      clearInterval(interval);
      unlisten.then(f => f());
    };
  }, []);

  const removeConversion = (id: string) => {
    setConversions(prev => prev.filter(c => c.id !== id));
  };

  const clearFinishedConversions = () => {
    setConversions(prev => prev.filter(c => c.status === "converting" || c.status === "pending"));
  };

  const cancelConversion = async (id: string) => {
    try {
      await invoke("cancel_conversion", { taskId: id });
      addLog("info", `Cancelled: ${getFileName(conversions.find(c => c.id === id)?.inputFile || "")}`);
    } catch (err) {
      console.error("Failed to cancel conversion:", err);
      addLog("error", `Failed to cancel conversion: ${err}`);
    }
  };

  const addBackToQueue = (conversion: ConversionItem) => {
    const file: QueueFile = {
      path: conversion.inputFile,
      name: getFileName(conversion.inputFile),
    };
    setQueue(prev => [...prev, file]);
    removeConversion(conversion.id);
    addLog("info", `Added back to queue: ${file.name}`);
  };

  const openFileLocation = async (filePath: string) => {
    try {
      await invoke("open_file_location", { filePath });
    } catch (err) {
      console.error("Failed to open file location:", err);
      addLog("error", `Failed to open file location: ${err}`);
    }
  };

  const getEncoderType = (enc: Encoder): string => {
    switch (enc.encoder_type) {
      case "GpuNvidia": return "NVIDIA GPU";
      case "GpuAmd": return "AMD GPU";
      case "GpuIntel": return "Intel GPU";
      case "Adobe": return "Professional";
      case "Cpu": return "CPU";
      default: return "Unknown";
    }
  };

  const getGpuTypeLabel = (gpuType: GpuType): string => {
    switch (gpuType) {
      case "Nvidia":
        return "NVIDIA";
      case "Amd":
        return "AMD";
      case "Intel":
        return "Intel";
      case "Unknown":
        return "Unknown";
      case "None":
        return "None";
      default:
        return "Unknown";
    }
  };

  const encoderMatchesGpuType = (enc: Encoder, gpuType: GpuType) => {
    if (gpuType === "Nvidia") return enc.encoder_type === "GpuNvidia";
    if (gpuType === "Amd") return enc.encoder_type === "GpuAmd";
    if (gpuType === "Intel") return enc.encoder_type === "GpuIntel";
    return false;
  };

  const isCpuLikeEncoder = (enc: Encoder) =>
    enc.encoder_type === "Cpu" || enc.encoder_type === "Adobe";

  const pickDefaultEncoder = (available: Encoder[], preferredGpuType?: GpuType) => {
    if (available.length === 0) return "";

    const preferredH264 = preferredGpuType
      ? available.find(enc => encoderMatchesGpuType(enc, preferredGpuType) && enc.codec === "h264")
      : undefined;
    if (preferredH264) return preferredH264.name;

    const preferredGpu = preferredGpuType
      ? available.find(enc => encoderMatchesGpuType(enc, preferredGpuType))
      : undefined;
    if (preferredGpu) return preferredGpu.name;

    const libx264 = available.find(enc => enc.name === "libx264");
    if (libx264) return libx264.name;

    const cpu = available.find(enc => isCpuLikeEncoder(enc));
    if (cpu) return cpu.name;

    return available[0].name;
  };

  const resolvePreferredGpuType = (preference: string, info: GpuInfo | null): GpuType | null => {
    if (!info) return null;
    if (preference === "auto") return info.gpu_type !== "None" ? info.gpu_type : null;
    if (preference === "cpu") return null;

    const selectedAdapter = info.adapters.find(adapter => adapter.id === preference);
    return selectedAdapter?.gpu_type ?? null;
  };

  const getFilteredEncoders = (available: Encoder[], info: GpuInfo | null, preference: string): Encoder[] => {
    if (available.length === 0) return [];

    if (preference === "cpu") {
      const cpuOnly = available.filter(isCpuLikeEncoder);
      return cpuOnly.length > 0 ? cpuOnly : available;
    }

    const preferredGpuType = resolvePreferredGpuType(preference, info);
    if (!preferredGpuType) {
      return available;
    }

    const targetGpuAndCpu = available.filter(
      enc => isCpuLikeEncoder(enc) || encoderMatchesGpuType(enc, preferredGpuType)
    );

    return targetGpuAndCpu.length > 0 ? targetGpuAndCpu : available.filter(isCpuLikeEncoder);
  };

  const getNvencIndexForSelection = (
    info: GpuInfo | null,
    preference: string,
    selectedEncoder: string
  ): number | undefined => {
    if (!info || !selectedEncoder.includes("nvenc")) return undefined;

    const nvidiaAdapters = info.adapters.filter(adapter => adapter.gpu_type === "Nvidia");
    if (nvidiaAdapters.length === 0) return undefined;

    if (preference === "auto") {
      const autoId = info.primary_adapter_id;
      if (autoId) {
        const autoIndex = nvidiaAdapters.findIndex(adapter => adapter.id === autoId);
        if (autoIndex >= 0) {
          return autoIndex;
        }
      }
      return 0;
    }
    if (preference === "cpu") return undefined;

    const selected = nvidiaAdapters.findIndex(adapter => adapter.id === preference);
    if (selected < 0) return undefined;
    return selected;
  };

  const getFileName = (path: string) => {
    return path.split(/[/\\]/).pop() || path;
  };

  const addFilesToQueue = (paths: string[]) => {
    if (paths.length === 0) return;

    const uniquePaths = Array.from(new Set(paths));
    const validPaths = uniquePaths.filter((path) => {
      const ext = getFileExt(getFileName(path));
      return SUPPORTED_INPUT_EXTENSIONS.has(ext);
    });
    const skippedCount = uniquePaths.length - validPaths.length;
    if (skippedCount > 0) {
      addLog("warn", `Skipped ${skippedCount} unsupported file(s).`);
    }
    if (validPaths.length === 0) return;

    const newFiles: QueueFile[] = validPaths.map((path) => ({
      path,
      name: getFileName(path),
    }));

    setQueue((prev) => {
      const existingPaths = new Set(prev.map((file) => file.path));
      const merged = [...prev];

      for (const file of newFiles) {
        if (!existingPaths.has(file.path)) {
          merged.push(file);
          existingPaths.add(file.path);
        }
      }

      return merged;
    });
  };

  const getFileExt = (name: string) => {
    const lastDot = name.lastIndexOf(".");
    return lastDot > 0 ? name.slice(lastDot + 1).toLowerCase() : "";
  };

  const getFileBase = (name: string) => {
    const lastDot = name.lastIndexOf(".");
    return lastDot > 0 ? name.slice(0, lastDot) : name;
  };

  const joinPath = (dir: string, file: string) => {
    const cleanDir = dir.replace(/[\\/]+$/, "");
    return `${cleanDir}\\${file}`;
  };

  const normalizeStatus = (status: unknown): ConversionStatus => {
    if (typeof status === "string") {
      switch (status) {
        case "Pending":
          return "pending";
        case "Running":
          return "converting";
        case "Completed":
          return "completed";
        case "Cancelled":
          return "cancelled";
        default:
          return "converting";
      }
    }
    if (status && typeof status === "object") {
      if ("Failed" in status) return "failed";
      if ("Cancelled" in status) return "cancelled";
    }
    return "converting";
  };

  const getFailureMessage = (status: unknown) => {
    if (status && typeof status === "object" && "Failed" in status) {
      const value = (status as { Failed?: unknown }).Failed;
      if (typeof value === "string" && value.trim().length > 0) {
        return value;
      }
    }
    return null;
  };

  const getLogFailureMessage = (log?: string[]) => {
    if (!log || log.length === 0) return null;
    const reversed = [...log].reverse();
    const keywordLine = reversed.find(line =>
      /(error|failed|invalid|unknown|could not|no such|permission|denied)/i.test(line)
    );
    return (keywordLine || reversed[0] || "").trim() || null;
  };

  const gpuPreferenceOptions = useMemo<GpuPreferenceOption[]>(() => {
    const options: GpuPreferenceOption[] = [];
    const primaryName = gpuInfo?.name || "Detected device";
    options.push({ value: "auto", label: `Auto (${primaryName})` });
    options.push({ value: "cpu", label: "CPU only (software)" });

    if (gpuInfo) {
      for (const adapter of gpuInfo.adapters) {
        const vendor =
          adapter.gpu_type === "Nvidia"
            ? "NVIDIA"
            : adapter.gpu_type === "Amd"
            ? "AMD"
            : adapter.gpu_type === "Intel"
            ? "Intel"
            : "GPU";
        options.push({
          value: adapter.id,
          label: `${vendor} - ${adapter.name}`,
        });
      }
    }

    return options;
  }, [gpuInfo]);

  const pollProgress = async () => {
    const active = conversionsRef.current.filter(
      c => c.status === "converting" || c.status === "pending"
    );
    if (active.length === 0) return;

    const updates = await Promise.all(
      active.map(async (conversion) => {
        try {
          const progress = await invoke<ConversionProgress | null>("get_conversion_progress", {
            taskId: conversion.id,
          });
          if (!progress) return null;
          const status = normalizeStatus(progress.status);
          const failureMessage =
            progress.error_message ??
            getFailureMessage(progress.status) ??
            getLogFailureMessage(progress.log);
          return {
            id: conversion.id,
            status,
            progress: typeof progress.percentage === "number" ? progress.percentage : conversion.progress,
            failureMessage,
          };
        } catch (err) {
          console.error("Failed to poll progress:", err);
          return {
            id: conversion.id,
            status: "failed" as ConversionStatus,
            progress: conversion.progress,
            failureMessage: String(err),
          };
        }
      })
    );

    const requeue: QueueFile[] = [];

    setConversions(prev => {
      const nextConversions: ConversionItem[] = [];

      prev.forEach(conversion => {
        const update = updates.find(item => item && item.id === conversion.id);
        if (!update) {
          nextConversions.push(conversion);
          return;
        }

        const next = {
          ...conversion,
          status: update.status,
          progress: Math.max(conversion.progress, update.progress),
          failureMessage: update.failureMessage ?? conversion.failureMessage,
        };

        if (conversion.status !== next.status) {
          if (next.status === "completed") {
            addLog("info", `Completed: ${getFileName(conversion.inputFile)}`);
          } else if (next.status === "failed") {
            const details = update.failureMessage ? ` (${update.failureMessage})` : "";
            addLog("error", `Failed: ${getFileName(conversion.inputFile)}${details}`);
          } else if (next.status === "cancelled") {
            addLog("info", `Cancelled: ${getFileName(conversion.inputFile)}`);
          }
        }

        if (next.status === "failed" || next.status === "cancelled") {
          requeue.push({
            path: conversion.inputFile,
            name: getFileName(conversion.inputFile),
          });
          return;
        }

        nextConversions.push(next);
      });

      return nextConversions;
    });

    if (requeue.length > 0) {
      setQueue(prev => {
        const merged = [...prev];
        for (const file of requeue) {
          if (!merged.some(item => item.path === file.path)) {
            merged.push(file);
          }
        }
        return merged;
      });
      const failedCount = requeue.length;
      setErrorMessage(`Failed to convert ${failedCount} item(s). Returned to queue.`);
    }
  };

  useEffect(() => {
    const hasActive = conversions.some(
      c => c.status === "converting" || c.status === "pending"
    );
    if (hasActive && pollerRef.current === null) {
      pollerRef.current = window.setInterval(() => {
        void pollProgress();
      }, 1000);
    }
    if (!hasActive && pollerRef.current !== null) {
      clearInterval(pollerRef.current);
      pollerRef.current = null;
    }

    return () => {
      if (pollerRef.current !== null && !hasActive) {
        clearInterval(pollerRef.current);
        pollerRef.current = null;
      }
    };
  }, [conversions]);

  // Load GPU encoders on mount
  useEffect(() => {
    const initializeApp = async () => {
      const cached = readHardwareCache();

      if (cached) {
        setCpuInfo(cached.cpuInfo);
        setGpuInfo(cached.gpuInfo);
        setGpuName(cached.gpuInfo.name || "");
        setAllEncoders(cached.gpuInfo.available_encoders);
        setIsHardwareInitializing(false);
        setHardwareInitError(null);
        addLog("info", "Loaded cached hardware profile. Refreshing in background...");
      } else {
        setIsHardwareInitializing(true);
        addLog("info", "Detecting CPU, GPUs, and encoders...");
      }

      setHardwareInitError(null);
      console.log("Fetching CPU and GPU info...");

      let latestCpuInfo = cached?.cpuInfo ?? null;
      let latestGpuInfo = cached?.gpuInfo ?? null;

      const cpuTask = invoke<CpuInfo>("get_cpu_info")
        .then((info) => {
          latestCpuInfo = info;
          setCpuInfo(info);
          addLog("info", `CPU: ${info.name} (${info.logical_cores} logical cores)`);
          if (latestGpuInfo) {
            writeHardwareCache(info, latestGpuInfo);
          }
        })
        .catch((err) => {
          console.error("Failed to get CPU info:", err);
          if (!cached) {
            setCpuInfo(null);
          }
          addLog("error", `CPU detection failed: ${String(err)}`);
        });

      try {
        const info = await invoke<GpuInfo>("get_gpu_info");
        latestGpuInfo = info;
        console.log("GPU info received:", info);
        console.log("Available encoders:", info.available_encoders);

        setGpuInfo(info);
        setGpuName(info.name || "");
        setAllEncoders(info.available_encoders);
        writeHardwareCache(latestCpuInfo, info);

        if (info.adapters.length === 0) {
          addLog("info", "GPU: no physical adapters detected");
        } else {
          addLog("info", `GPU adapters detected: ${info.adapters.length}`);
          for (const adapter of info.adapters) {
            const primarySuffix =
              info.primary_adapter_id === adapter.id ? " [primary]" : "";
            addLog(
              "info",
              `GPU ${adapter.id}: ${adapter.name} (${getGpuTypeLabel(adapter.gpu_type)})${primarySuffix}`
            );
          }
        }
        if (info.available_encoders.length === 0) {
          setHardwareInitError("No encoders were detected.");
        }
      } catch (err) {
        console.error("Failed to get GPU info:", err);
        if (!cached) {
          setGpuName("Detection failed: " + String(err));
          setGpuInfo(null);
          setAllEncoders([]);
          setHardwareInitError(String(err));
        }
        addLog("error", `GPU detection failed: ${String(err)}`);
      } finally {
        setIsHardwareInitializing(false);
        addLog("info", "Hardware detection completed.");
      }

      void cpuTask;
    };

    void initializeApp();
  }, []);

  useEffect(() => {
    if (gpuPreference === "auto" || gpuPreference === "cpu") return;
    if (!gpuInfo) return;

    const exists = gpuInfo.adapters.some((adapter) => adapter.id === gpuPreference);
    if (!exists) {
      setGpuPreference("auto");
    }
  }, [gpuInfo, gpuPreference]);

  useEffect(() => {
    const filtered = getFilteredEncoders(allEncoders, gpuInfo, gpuPreference);
    setEncoders(filtered);

    setEncoder(prev => {
      if (prev && filtered.some(enc => enc.name === prev)) {
        return prev;
      }

      const preferredType = resolvePreferredGpuType(gpuPreference, gpuInfo) ?? undefined;
      return pickDefaultEncoder(filtered, preferredType);
    });
  }, [allEncoders, gpuInfo, gpuPreference]);

  useEffect(() => {
    const setDefaultOutputDir = async () => {
      if (outputDir) return;
      try {
        const target = await invoke<string>("get_default_output_dir");
        setOutputDir(target);
      } catch (err) {
        console.warn("Failed to resolve default output directory:", err);
      }
    };

    setDefaultOutputDir();
  }, [outputDir]);

  useEffect(() => {
    const appWindow = getCurrentWindow();
    let unlisten: (() => void) | null = null;
    let active = true;

    void appWindow.onDragDropEvent((event) => {
      if (!active) return;
      const payload = event.payload;

      if (payload.type === "enter") {
        setIsDragOverlayVisible(true);
        setDraggedFileCount(payload.paths.length);
        return;
      }

      if (payload.type === "over") {
        setIsDragOverlayVisible(true);
        return;
      }

      if (payload.type === "leave") {
        setIsDragOverlayVisible(false);
        setDraggedFileCount(0);
        return;
      }

      if (payload.type === "drop") {
        setIsDragOverlayVisible(false);
        setDraggedFileCount(0);

        if (payload.paths.length > 0) {
          addFilesToQueue(payload.paths);
          addLog("info", `Added ${payload.paths.length} file(s) via drag and drop.`);
        }
      }
    }).then((fn) => {
      if (!active) {
        fn();
        return;
      }
      unlisten = fn;
    }).catch((err) => {
      addLog("error", `Drag and drop initialization failed: ${String(err)}`);
    });

    return () => {
      active = false;
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  const handleSelectOutputDir = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
      });
      if (selected && typeof selected === "string") {
        setOutputDir(selected);
      }
    } catch (err) {
      console.error("Failed to select directory:", err);
    }
  };

  const startPendingConversion = async (item: ConversionItem): Promise<string | null> => {
    if (!item.params) return null;
    try {
      const taskId = await invoke<string>("start_conversion", {
        args: {
          inputFile: item.inputFile,
          outputFile: item.outputFile,
          encoder: item.params.encoder,
          gpuIndex: item.params.gpuIndex,
          cpuThreads: item.params.cpuThreads,
          preset: item.params.preset,
          isAdobePreset: false,
        },
      });
      return taskId;
    } catch (err) {
      addLog("error", `Failed to start: ${getFileName(item.inputFile)} (${String(err)})`);
      return null;
    }
  };

  const startNextPending = async () => {
    const current = conversionsRef.current;
    const activeCount = current.filter(c => c.status === "converting").length;
    const pending = current.filter(c => c.status === "pending" && c.params);
    const slotsAvailable = maxConcurrent - activeCount;

    for (let i = 0; i < Math.min(slotsAvailable, pending.length); i++) {
      const item = pending[i];
      const taskId = await startPendingConversion(item);
      if (taskId) {
        setConversions(prev => prev.map(c =>
          c.id === item.id ? { ...c, id: taskId, status: "converting" as ConversionStatus } : c
        ));
      } else {
        setConversions(prev => prev.map(c =>
          c.id === item.id ? { ...c, status: "failed" as ConversionStatus, failureMessage: "Failed to start" } : c
        ));
      }
    }
  };

  const handleStartConversion = async () => {
    if (isHardwareInitializing) {
      addLog("warn", "Please wait for hardware detection to finish.");
      return;
    }

    if (queue.length === 0) {
      addLog("warn", "Queue is empty.");
      return;
    }

    const selectedEncoder = encoder || encoders[0]?.name || "libx264";
    if (!encoder) {
      setEncoder(selectedEncoder);
    }
    const selectedGpuOption = gpuPreferenceOptions.find(option => option.value === gpuPreference);
    const gpuIndex = getNvencIndexForSelection(gpuInfo, gpuPreference, selectedEncoder);
    const cpuThreads = cpuLimit < 100 && cpuInfo
      ? Math.max(1, Math.round(cpuInfo.logical_cores * cpuLimit / 100))
      : undefined;

    setErrorMessage(null);

    let ffmpegReady = false;
    try {
      const status = await invoke<FfmpegStatus>("check_ffmpeg");
      ffmpegReady = status.available;
      if (!ffmpegReady) {
        addLog("warn", "FFmpeg not found. Downloading...");
        await invoke<string>("download_ffmpeg");
        ffmpegReady = true;
        addLog("info", "FFmpeg downloaded.");
      }
    } catch (err) {
      addLog("error", `FFmpeg check failed: ${String(err)}`);
    }

    if (!ffmpegReady) {
      addLog("error", "Cannot start conversions without FFmpeg.");
      return;
    }

    const encoderInfo = encoders.find(e => e.name === selectedEncoder);
    if (encoderInfo) {
      addLog("info", `Encoder: ${encoderInfo.description} (${getEncoderType(encoderInfo)})`);
    }
    if (selectedGpuOption) {
      addLog("info", `GPU preference: ${selectedGpuOption.label}`);
    }
    if (typeof gpuIndex === "number") {
      const nvidiaAdapters = gpuInfo?.adapters.filter(a => a.gpu_type === "Nvidia") ?? [];
      const gpuLabel = nvidiaAdapters[gpuIndex]?.name ?? `NVIDIA device ${gpuIndex}`;
      addLog("info", `NVENC GPU: ${gpuLabel} (device ${gpuIndex})`);
    }
    if (outputFormat) {
      addLog("info", `Output format: ${outputFormat.toUpperCase()}`);
    }
    if (cpuThreads) {
      addLog("info", `CPU limit: ${cpuLimit}% (${cpuThreads} threads)`);
    }
    addLog("info", `Max concurrent: ${maxConcurrent}`);

    addLog("info", `Starting ${queue.length} conversion(s)...`);
    setActiveTab("progress");

    let resolvedOutputDir = outputDir;
    if (!resolvedOutputDir) {
      try {
        const autoOutputDir = await invoke<string>("get_default_output_dir");
        resolvedOutputDir = autoOutputDir;
        setOutputDir(autoOutputDir);
        addLog("info", `Using default output directory: ${autoOutputDir}`);
      } catch (err) {
        addLog("error", `Failed to resolve default output directory: ${String(err)}`);
      }
    }

    if (!resolvedOutputDir) {
      const message = "No output directory available. Please select one in Settings.";
      setErrorMessage(message);
      addLog("error", message);
      setActiveTab("queue");
      return;
    }

    // Build all conversion items as pending
    const params: ConversionParams = { encoder: selectedEncoder, gpuIndex, cpuThreads, preset };
    const newItems: ConversionItem[] = [];

    for (const file of queue) {
      const inputFile = file.path;
      const baseName = getFileBase(getFileName(inputFile));
      const ext = outputFormat || getFileExt(getFileName(inputFile)) || "mp4";
      const outputFile = joinPath(resolvedOutputDir, `${baseName}_converted.${ext}`);

      newItems.push({
        id: `pending-${Date.now()}-${Math.random().toString(36).slice(2)}`,
        inputFile,
        outputFile,
        status: "pending",
        progress: 0,
        params,
      });
    }

    setConversions(prev => [...prev, ...newItems]);
    setQueue([]);

    // Start up to maxConcurrent immediately
    let started = 0;
    for (const item of newItems) {
      if (started >= maxConcurrent) break;
      const taskId = await startPendingConversion(item);
      if (taskId) {
        setConversions(prev => prev.map(c =>
          c.id === item.id ? { ...c, id: taskId, status: "converting" as ConversionStatus } : c
        ));
        started++;
      } else {
        setConversions(prev => prev.map(c =>
          c.id === item.id ? { ...c, status: "failed" as ConversionStatus, failureMessage: "Failed to start" } : c
        ));
      }
    }
  };

  const handleAddFiles = async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [
          { name: "Video files", extensions: ["mkv", "mp4", "avi", "mov", "wmv", "flv", "webm"] },
          { name: "All files", extensions: ["*"] },
        ],
      });

      if (selected) {
        const files = Array.isArray(selected) ? selected : [selected];
        addFilesToQueue(files);
      }
    } catch (err) {
      console.error("Failed to select files:", err);
    }
  };

  const handleRemoveFile = (index: number) => {
    setQueue(prev => prev.filter((_, i) => i !== index));
  };

  const handleClearQueue = () => {
    setQueue([]);
  };

  return (
    <>
      <StarBackground />
      {isDragOverlayVisible && (
        <div className="drag-overlay" aria-hidden="true">
          <div className="drag-overlay-panel">
            <i className="ri-file-upload-fill drag-overlay-icon"></i>
            <h2 className="drag-overlay-title">
              {draggedFileCount > 0
                ? `Drop ${draggedFileCount} file${draggedFileCount > 1 ? "s" : ""} here`
                : "Drop files here"}
            </h2>
            <p className="drag-overlay-subtitle">Release to add files to the queue</p>
          </div>
        </div>
      )}

      <div className="app">
        <header className="header">
          <div>
            <h1>Dreamcodec</h1>
            <p>Hardware-accelerated video conversion</p>
          </div>
          <div className="gpu-badge">
            <i className="ri-dashboard-fill"></i>
            <span>{isHardwareInitializing ? "Detecting hardware..." : (gpuName || "CPU (software)")}</span>
          </div>
        </header>

        <div className="main">
          <aside className="sidebar">
            <h2>Settings</h2>

            <div className="form-group">
              <label><i className="ri-folder-fill"></i> Output Directory</label>
              <div className="input-group">
                <input
                  type="text"
                  className="input"
                  placeholder="Select output folder..."
                  value={outputDir}
                  readOnly
                />
                <button className="button button-icon button-icon-only" onClick={handleSelectOutputDir} title="Browse">
                  <i className="ri-folder-open-fill"></i>
                </button>
              </div>
            </div>

            <div className="form-group">
              <label><i className="ri-cpu-fill"></i> Preferred GPU</label>
              <select
                className="select"
                value={gpuPreference}
                onChange={(e) => setGpuPreference(e.target.value)}
                disabled={isHardwareInitializing}
              >
                {gpuPreferenceOptions.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
              <p className="help-text">
                {isHardwareInitializing
                  ? "Detecting adapters..."
                  : "Auto prefers the best detected GPU; choose CPU for maximum compatibility."}
              </p>
            </div>

            <div className="form-group">
              <label><i className="ri-movie-2-fill"></i> Video Encoder</label>
              <select
                className="select"
                value={encoder}
                onChange={(e) => setEncoder(e.target.value)}
                disabled={isHardwareInitializing}
              >
                <option value="">Select encoder...</option>
                {encoders.map((enc) => (
                  <option key={enc.name} value={enc.name}>
                    {enc.description} ({getEncoderType(enc)})
                  </option>
                ))}
              </select>
              {isHardwareInitializing ? (
                <p className="help-text">
                  <i className="ri-loader-4-line icon-spin"></i> Detecting available encoders...
                </p>
              ) : encoders.length === 0 && (
                <p className="help-text" style={{ color: "rgba(239, 68, 68, 0.7)" }}>
                  <i className="ri-error-warning-fill"></i>{" "}
                  {hardwareInitError
                    ? `No encoders detected (${hardwareInitError}).`
                    : "No encoders detected. FFmpeg may not be installed."}
                </p>
              )}
            </div>

            <div className="form-group">
              <label><i className="ri-file-transfer-fill"></i> Output Format</label>
              <select className="select" value={outputFormat} onChange={(e) => setOutputFormat(e.target.value)}>
                <optgroup label="Video">
                  <option value="mp4">MP4</option>
                  <option value="mkv">MKV</option>
                  <option value="avi">AVI</option>
                  <option value="mov">MOV</option>
                </optgroup>
                <optgroup label="Audio Only">
                  <option value="mp3">MP3</option>
                  <option value="wav">WAV</option>
                  <option value="aac">AAC</option>
                  <option value="flac">FLAC</option>
                  <option value="m4a">M4A</option>
                </optgroup>
              </select>
              <p className="help-text">Choose the target file extension for conversion</p>
            </div>

            <div className="form-group">
              <label><i className="ri-speed-fill"></i> Preset</label>
              <select className="select" value={preset} onChange={(e) => setPreset(e.target.value)}>
                <option value="ultrafast">Ultra Fast</option>
                <option value="superfast">Super Fast</option>
                <option value="veryfast">Very Fast</option>
                <option value="faster">Faster</option>
                <option value="fast">Fast</option>
                <option value="medium">Medium</option>
                <option value="slow">Slow</option>
                <option value="slower">Slower</option>
                <option value="veryslow">Very Slow</option>
              </select>
              <p className="help-text">Faster = larger files, Slower = better compression</p>
            </div>

            <div className="form-group">
              <label><i className="ri-cpu-line"></i> CPU Limit</label>
              <select className="select" value={cpuLimit} onChange={(e) => setCpuLimit(Number(e.target.value))}>
                <option value={100}>100% (No limit)</option>
                <option value={75}>75%</option>
                <option value={50}>50%</option>
                <option value={25}>25%</option>
              </select>
              <p className="help-text">Limit CPU threads per conversion to keep your system responsive</p>
            </div>

            <div className="form-group">
              <label><i className="ri-stack-fill"></i> Max Concurrent</label>
              <select className="select" value={maxConcurrent} onChange={(e) => setMaxConcurrent(Number(e.target.value))}>
                <option value={1}>1</option>
                <option value={2}>2</option>
                <option value={3}>3 (Default)</option>
                <option value={4}>4</option>
                <option value={5}>5</option>
              </select>
              <p className="help-text">Maximum files converting at the same time</p>
            </div>

            <button className="button button-add-files" onClick={handleAddFiles}>
              <i className="ri-add-line"></i> Add Files
            </button>
          </aside>

          <div className="content">
            <div className="tabs">
              <button
                className={`tab ${activeTab === "queue" ? "active" : ""}`}
                onClick={() => setActiveTab("queue")}
              >
                <i className="ri-file-list-3-fill"></i> Queue ({queue.length})
              </button>
              <button
                className={`tab ${activeTab === "progress" ? "active" : ""}`}
                onClick={() => setActiveTab("progress")}
              >
                <i className="ri-loader-4-fill"></i> Progress
              </button>
              <button
                className={`tab ${activeTab === "logs" ? "active" : ""}`}
                onClick={() => setActiveTab("logs")}
              >
                <i className="ri-file-text-fill"></i> Logs
              </button>
            </div>

            <div className="tab-content">
              {panicInfo && (
                <div className="error-banner error-banner-wide">
                  <i className="ri-error-warning-fill"></i>
                  <span>A critical error occurred: {panicInfo.payload} at {panicInfo.location}</span>
                </div>
              )}
              {errorMessage && (
                <div className="error-banner error-banner-wide">
                  <i className="ri-error-warning-fill"></i>
                  <span>{errorMessage}</span>
                </div>
              )}
              {activeTab === "queue" && (
                <div className="queue-panel">
                  {queue.length > 0 && (
                    <div className="queue-header">
                      <button className="button" onClick={handleClearQueue}>
                        <i className="ri-delete-bin-fill"></i> Clear All
                      </button>
                      <button
                        className="button button-start"
                        onClick={handleStartConversion}
                        disabled={queue.length === 0 || isHardwareInitializing}
                      >
                        <i className="ri-play-circle-fill"></i> Start Conversion
                      </button>
                    </div>
                  )}

                  {queue.length === 0 ? (
                    <div className="empty-state">
                      <i className="ri-folder-open-line empty-icon"></i>
                      <h3>No files in queue</h3>
                      <p>Click "Add Files" to select videos for conversion</p>
                    </div>
                  ) : (
                    <div className="file-list">
                      {queue.map((file, index) => (
                        <div key={index} className="file-item">
                          <i className="ri-movie-fill file-icon"></i>
                          <div className="file-info">
                            <div className="file-name">{file.name}</div>
                            <div className="file-path">{file.path}</div>
                          </div>
                          <button
                            className="button button-small button-danger button-icon-only"
                            onClick={() => handleRemoveFile(index)}
                            title="Remove"
                          >
                            <i className="ri-close-line"></i>
                          </button>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}

              {activeTab === "progress" && (
                conversions.length === 0 ? (
                  <div className="empty-state">
                    <i className="ri-loader-4-line empty-icon"></i>
                    <h3>No active conversions</h3>
                    <p>Start a conversion to see progress here</p>
                  </div>
                ) : (
                  <>
                    <div className="progress-header">
                      <div className="progress-meta">
                        <span>{conversions.length} item(s)</span>
                      </div>
                      {conversions.some(c => c.status !== "converting" && c.status !== "pending") && (
                        <button className="button button-small" onClick={clearFinishedConversions}>
                          <i className="ri-delete-bin-6-line"></i> Clear Finished
                        </button>
                      )}
                    </div>
                    <div className="file-list">
                      {conversions.map((conversion) => (
                        <div key={conversion.id} className="file-item">
                          <i className={`file-icon ${
                            conversion.status === "converting" || conversion.status === "pending"
                              ? "ri-loader-4-line icon-spin"
                              : conversion.status === "completed"
                              ? "ri-checkbox-circle-fill icon-success"
                              : conversion.status === "failed"
                              ? "ri-close-circle-fill icon-error"
                              : "ri-checkbox-circle-line"
                          }`}></i>
                          <div className="file-info">
                            <div className="file-name">{getFileName(conversion.inputFile)}</div>
                            <div className="file-path">{conversion.outputFile}</div>
                            <div className="progress-bar">
                              <div
                                className="progress-bar-fill"
                                style={{ width: `${conversion.progress}%` }}
                              />
                            </div>
                          </div>
                          <div className="conversion-status">
                          <div className={`conversion-status-text ${
                            conversion.status === "completed" ? "status-success" :
                            conversion.status === "failed" ? "status-error" :
                            ""
                          }`}>{conversion.status}</div>
                          <div className="conversion-status-progress">
                            {conversion.progress.toFixed(1)}%
                          </div>
                          {conversion.status === "failed" && conversion.failureMessage && (
                            <div className="conversion-error" title={conversion.failureMessage}>
                              {conversion.failureMessage}
                            </div>
                          )}
                          {(conversion.status === "converting" || conversion.status === "pending") && (
                            <button
                              className="button button-small button-danger button-icon-only conversion-remove"
                              onClick={() => cancelConversion(conversion.id)}
                              title="Cancel"
                            >
                                <i className="ri-close-line"></i>
                              </button>
                            )}
                          {conversion.status === "completed" && (
                            <>
                              <button
                                className="button button-small button-primary button-icon-only conversion-requeue"
                                onClick={() => addBackToQueue(conversion)}
                                title="Add back to queue"
                              >
                                  <i className="ri-arrow-go-back-line"></i>
                                </button>
                              <button
                                className="button button-small button-icon-only"
                                onClick={() => openFileLocation(conversion.outputFile)}
                                title="Open file location"
                              >
                                  <i className="ri-folder-open-line"></i>
                                </button>
                            </>
                            )}
                          {conversion.status !== "converting" && conversion.status !== "pending" && conversion.status !== "completed" && (
                            <button
                              className="button button-small button-danger button-icon-only conversion-remove"
                              onClick={() => removeConversion(conversion.id)}
                              title="Remove"
                            >
                                <i className="ri-close-line"></i>
                              </button>
                            )}
                          </div>
                        </div>
                      ))}
                    </div>
                  </>
                )
              )}

              {activeTab === "logs" && (
                <div className="logs-panel">
                  {logs.map((entry, index) => (
                    <div key={`${entry}-${index}`} className="log-entry">{entry}</div>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>

        <footer className="footer">
          <i className="ri-video-fill"></i> Dreamcodec v2.2.6 • Made by Thornvald
        </footer>
      </div>
    </>
  );
}
