import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import StarBackground from "./components/StarBackground";
import "remixicon/fonts/remixicon.css";

type EncoderType = "Cpu" | "GpuNvidia" | "GpuAmd" | "GpuIntel" | "Adobe";

interface Encoder {
  name: string;
  description: string;
  codec: string;
  encoder_type: EncoderType;
}

interface GpuInfo {
  detected: boolean;
  gpu_type: string;
  name: string;
  available_encoders: Encoder[];
}

interface FfmpegStatus {
  available: boolean;
  path?: string;
  version?: string;
  source?: string;
}

interface QueueFile {
  path: string;
  name: string;
}

type ConversionStatus = "pending" | "converting" | "completed" | "failed" | "cancelled";

interface ConversionItem {
  id: string;
  inputFile: string;
  outputFile: string;
  status: ConversionStatus;
  progress: number;
  failureMessage?: string | null;
}

interface ConversionProgress {
  status: unknown;
  percentage: number;
  log?: string[];
  error_message?: string | null;
}

export default function App() {
  const [activeTab, setActiveTab] = useState("queue");
  const [outputDir, setOutputDir] = useState("");
  const [encoder, setEncoder] = useState("");
  const [preset, setPreset] = useState("fast");
  const [outputFormat, setOutputFormat] = useState("mp4");
  const [queue, setQueue] = useState<QueueFile[]>([]);
  const [encoders, setEncoders] = useState<Encoder[]>([]);
  const [gpuName, setGpuName] = useState("");
  const [conversions, setConversions] = useState<ConversionItem[]>([]);
  const [logs, setLogs] = useState<string[]>([]);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const conversionsRef = useRef<ConversionItem[]>([]);
  const pollerRef = useRef<number | null>(null);

  useEffect(() => {
    conversionsRef.current = conversions;
  }, [conversions]);

  const addLog = (message: string) => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs(prev => [...prev, `${timestamp} - ${message}`]);
  };

  const removeConversion = (id: string) => {
    setConversions(prev => prev.filter(c => c.id !== id));
  };

  const clearFinishedConversions = () => {
    setConversions(prev => prev.filter(c => c.status === "converting" || c.status === "pending"));
  };

  const cancelConversion = async (id: string) => {
    try {
      await invoke("cancel_conversion", { taskId: id });
      addLog(`Cancelled: ${getFileName(conversions.find(c => c.id === id)?.inputFile || "")}`);
    } catch (err) {
      console.error("Failed to cancel conversion:", err);
      addLog(`Failed to cancel conversion: ${err}`);
    }
  };

  const addBackToQueue = (conversion: ConversionItem) => {
    const file: QueueFile = {
      path: conversion.inputFile,
      name: getFileName(conversion.inputFile),
    };
    setQueue(prev => [...prev, file]);
    removeConversion(conversion.id);
    addLog(`Added back to queue: ${file.name}`);
  };

  const openFileLocation = async (filePath: string) => {
    try {
      await invoke("open_file_location", { filePath });
    } catch (err) {
      console.error("Failed to open file location:", err);
      addLog(`Failed to open file location: ${err}`);
    }
  };

  const isVirtualGpu = (name: string) => {
    return /(virtual|remote|basic display|microsoft basic|indirect display|displaylink|rdp|vmware|virtualbox|parallels|citrix|xen|dummy)/i.test(name);
  };

  const getFileName = (path: string) => {
    return path.split(/[/\\]/).pop() || path;
  };

  const getFileDir = (path: string) => {
    const parts = path.split(/[/\\]/);
    parts.pop();
    return parts.join("\\");
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
            addLog(`Completed: ${getFileName(conversion.inputFile)}`);
          } else if (next.status === "failed") {
            const details = update.failureMessage ? ` (${update.failureMessage})` : "";
            addLog(`Failed: ${getFileName(conversion.inputFile)}${details}`);
          } else if (next.status === "cancelled") {
            addLog(`Cancelled: ${getFileName(conversion.inputFile)}`);
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
      try {
        console.log("Fetching GPU info...");
        const info = await invoke<GpuInfo>("get_gpu_info");
        console.log("GPU info received:", info);
        console.log("Available encoders:", info.available_encoders);

        setEncoders(info.available_encoders);
        if (info.name && !isVirtualGpu(info.name)) {
          setGpuName(info.name);
        } else {
          setGpuName("");
        }

        // Prefer CPU encoder by default for reliability
        const cpuEncoder = info.available_encoders.find(e => e.encoder_type === "Cpu");
        if (cpuEncoder) {
          setEncoder(cpuEncoder.name);
        } else if (info.available_encoders.length > 0) {
          setEncoder(info.available_encoders[0].name);
        }
      } catch (err) {
        console.error("Failed to get GPU info:", err);
        setGpuName("Detection failed: " + String(err));
      }
    };

    initializeApp();
  }, []);

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

  const handleStartConversion = async () => {
    if (queue.length === 0) {
      addLog("Queue is empty.");
      return;
    }

    const selectedEncoder = encoder || encoders[0]?.name || "libx264";
    if (!encoder) {
      setEncoder(selectedEncoder);
    }

    setErrorMessage(null);

    let ffmpegReady = false;
    try {
      const status = await invoke<FfmpegStatus>("check_ffmpeg");
      ffmpegReady = status.available;
      if (!ffmpegReady) {
        addLog("FFmpeg not found. Downloading...");
        await invoke<string>("download_ffmpeg");
        ffmpegReady = true;
        addLog("FFmpeg downloaded.");
      }
    } catch (err) {
      addLog(`FFmpeg check failed: ${String(err)}`);
    }

    if (!ffmpegReady) {
      addLog("Cannot start conversions without FFmpeg.");
      return;
    }

    const encoderInfo = encoders.find(e => e.name === selectedEncoder);
    if (encoderInfo) {
      addLog(`Encoder: ${encoderInfo.description} (${getEncoderType(encoderInfo)})`);
    }
    if (outputFormat) {
      addLog(`Output format: ${outputFormat.toUpperCase()}`);
    }

    addLog(`Starting ${queue.length} conversion(s)...`);
    setActiveTab("progress");

    let startedCount = 0;
    const failures: QueueFile[] = [];

    for (const file of queue) {
      const inputFile = file.path;
      const baseName = getFileBase(getFileName(inputFile));
      const ext = outputFormat || getFileExt(getFileName(inputFile)) || "mp4";
      const targetDir = outputDir || getFileDir(inputFile);
      if (!targetDir) {
        addLog(`No output directory for: ${getFileName(inputFile)}`);
        failures.push(file);
        continue;
      }
      const outputFile = joinPath(targetDir, `${baseName}_converted.${ext}`);

      try {
        const taskId = await invoke<string>("start_conversion", {
          args: {
            inputFile,
            outputFile,
            encoder: selectedEncoder,
            preset,
            isAdobePreset: false,
          },
        });

        setConversions(prev => [
          ...prev,
          {
            id: taskId,
            inputFile,
            outputFile,
            status: "converting",
            progress: 0,
          },
        ]);
        startedCount += 1;
      } catch (err) {
        console.error("Failed to start conversion:", err);
        addLog(`Failed to start: ${getFileName(inputFile)} (${String(err)})`);
        failures.push(file);
      }
    }

    if (startedCount === queue.length) {
      setQueue([]);
    } else {
      setQueue(failures);
      setErrorMessage(`Failed to start ${failures.length} conversion(s). Check Logs for details.`);
      if (startedCount === 0) {
        setActiveTab("queue");
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
        const newFiles = files.map(path => ({
          path,
          name: path.split(/[/\\]/).pop() || path,
        }));
        setQueue(prev => [...prev, ...newFiles]);
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

  return (
    <>
      <StarBackground />

      <div className="app">
        <header className="header">
          <div>
            <h1>Dreamcodec</h1>
            <p>Hardware-accelerated video conversion</p>
          </div>
          <div className="gpu-badge">
            <i className="ri-dashboard-fill"></i>
            <span>{gpuName || "CPU (software)"}</span>
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
              <label><i className="ri-movie-2-fill"></i> Video Encoder</label>
              <select className="select" value={encoder} onChange={(e) => setEncoder(e.target.value)}>
                <option value="">Select encoder...</option>
                {encoders.map((enc) => (
                  <option key={enc.name} value={enc.name}>
                    {enc.description} ({getEncoderType(enc)})
                  </option>
                ))}
              </select>
              {encoders.length === 0 && (
                <p className="help-text" style={{ color: "rgba(239, 68, 68, 0.7)" }}>
                  <i className="ri-error-warning-fill"></i> No encoders detected. FFmpeg may not be installed.
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
                        disabled={queue.length === 0}
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
                  <div className="log-entry"><i className="ri-checkbox-circle-fill"></i> Application started</div>
                  <div className="log-entry">
                    <i className="ri-dashboard-fill"></i> GPU detected: <span className={gpuName ? "status-success" : "status-error"}>{gpuName || "No GPU detected"}</span>
                  </div>
                  <div className="log-entry"><i className="ri-list-check"></i> Encoders available: {encoders.length}</div>
                  {logs.map((entry, index) => (
                    <div key={`${entry}-${index}`} className="log-entry">{entry}</div>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>

        <footer className="footer">
          <i className="ri-video-fill"></i> Dreamcodec v2.2.1 • Made by Thornvald
        </footer>
      </div>
    </>
  );
}
