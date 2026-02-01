import { useState, useCallback } from "react";
import { Plus, Trash2, Play, FileVideo, FolderPlus, ArrowRight, Film, Music, Clapperboard } from "lucide-react";
import type { InputFormat, OutputFormat } from "../types";

interface FileQueueProps {
  files: string[];
  onAddFiles: () => void;
  onRemoveFile: (index: number) => void;
  onStartConversion: () => void;
  isConverting: boolean;
  inputFormat: InputFormat;
  outputFormat: OutputFormat;
}

export default function FileQueue({
  files,
  onAddFiles,
  onRemoveFile,
  onStartConversion,
  isConverting,
  inputFormat,
  outputFormat,
}: FileQueueProps) {
  const [dragOver, setDragOver] = useState(false);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
  }, []);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    // Handle file drop - would need to implement in parent
  }, []);

  const getFileName = (path: string) => {
    return path.split("/").pop() || path.split("\\").pop() || path;
  };

  const getFileExtension = (path: string) => {
    const name = getFileName(path);
    const ext = name.split(".").pop()?.toLowerCase();
    return ext;
  };

  const getFormatIcon = (format: string, type: "input" | "output") => {
    const isAudio = ["mp3", "wav", "aac", "flac", "m4a"].includes(format);
    const isPro = ["prores", "dnxhd"].includes(format);
    
    if (isAudio) {
      return <Music size={16} style={{ color: type === "input" ? '#555' : '#aaa' }} />;
    }
    if (isPro) {
      return <Clapperboard size={16} style={{ color: type === "input" ? '#555' : '#ccc' }} />;
    }
    return <Film size={16} style={{ color: type === "input" ? '#555' : '#888' }} />;
  };

  const getFormatBadgeClass = (format: string) => {
    const isAudio = ["mp3", "wav", "aac", "flac", "m4a"].includes(format);
    const isPro = ["prores", "dnxhd"].includes(format);
    
    if (isAudio) return "format-icon format-audio";
    if (isPro) return "format-icon format-pro";
    return "format-icon format-video";
  };

  const isAudioOutput = ["mp3", "wav", "aac", "flac", "m4a"].includes(outputFormat);
  const isProOutput = ["prores", "dnxhd"].includes(outputFormat);

  return (
    <div className="flex flex-col h-full p-4">
      {/* Format Info Bar */}
      <div 
        className="mb-4 p-3 rounded-lg border animate-fade-in"
        style={{ backgroundColor: 'rgba(26, 26, 26, 0.8)', borderColor: '#333' }}
      >
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <span className="text-sm" style={{ color: '#888' }}>Input:</span>
            <span 
              className="flex items-center gap-2 px-2 py-1 rounded text-sm"
              style={{ backgroundColor: '#222' }}
            >
              {getFormatIcon(inputFormat, "input")}
              <span className="uppercase font-medium" style={{ color: '#aaa' }}>{inputFormat}</span>
            </span>
            <ArrowRight size={16} style={{ color: '#555' }} />
            <span className="text-sm" style={{ color: '#888' }}>Output:</span>
            <span className={`flex items-center gap-2 px-2 py-1 rounded text-sm ${getFormatBadgeClass(outputFormat)}`}>
              {getFormatIcon(outputFormat, "output")}
              <span className="uppercase font-medium">{outputFormat}</span>
            </span>
          </div>
          <div className="text-xs" style={{ color: '#555' }}>
            {files.length} file{files.length !== 1 ? "s" : ""} in queue
          </div>
        </div>
      </div>

      {/* Action Buttons */}
      <div className="flex gap-3 mb-4">
        <button
          onClick={onAddFiles}
          className="btn btn-primary flex items-center gap-2"
          disabled={isConverting}
        >
          <Plus size={18} />
          Add Files
        </button>
        <button
          onClick={onStartConversion}
          className="btn btn-success flex items-center gap-2"
          disabled={files.length === 0 || isConverting}
        >
          <Play size={18} />
          Start Conversion
        </button>
      </div>

      {/* Drop Zone / File List */}
      <div
        className={`flex-1 border-2 border-dashed rounded-xl transition-all ${
          dragOver
            ? "border-gray-500"
            : ""
        }`}
        style={{ 
          borderColor: dragOver ? '#666' : '#333',
          backgroundColor: dragOver ? 'rgba(100, 100, 100, 0.1)' : 'rgba(17, 17, 17, 0.5)'
        }}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
      >
        {files.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full" style={{ color: '#555' }}>
            <FolderPlus size={64} className="mb-4 opacity-50" />
            <p className="text-lg font-medium">Drop video files here</p>
            <p className="text-sm">or click &quot;Add Files&quot; to browse</p>
            <p className="text-xs mt-2 opacity-70">
              Supports: {inputFormat.toUpperCase()}
            </p>
          </div>
        ) : (
          <div className="h-full overflow-y-auto p-2">
            <div className="space-y-2">
              {files.map((file, index) => (
                <div
                  key={index}
                  className="flex items-center gap-3 p-3 rounded-lg border card-glow animate-slide-in"
                  style={{ 
                    backgroundColor: '#1a1a1a', 
                    borderColor: '#333',
                    animationDelay: `${index * 50}ms`
                  }}
                >
                  <div className="flex-shrink-0">
                    <FileVideo size={24} style={{ color: '#888' }} />
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="font-medium truncate" style={{ color: '#e0e0e0' }}>{getFileName(file)}</p>
                    <div className="flex items-center gap-2 text-xs" style={{ color: '#555' }}>
                      <span className="uppercase">{getFileExtension(file)}</span>
                      <ArrowRight size={10} />
                      <span className={`uppercase font-medium ${
                        isAudioOutput ? "text-gray-400" : 
                        isProOutput ? "text-gray-300" : "text-gray-500"
                      }`}>
                        {outputFormat}
                      </span>
                    </div>
                  </div>
                  <button
                    onClick={() => onRemoveFile(index)}
                    className="p-2 rounded-lg transition-colors hover:bg-red-500/10"
                    style={{ color: '#888' }}
                    disabled={isConverting}
                    onMouseEnter={(e) => e.currentTarget.style.color = '#f87171'}
                    onMouseLeave={(e) => e.currentTarget.style.color = '#888'}
                  >
                    <Trash2 size={18} />
                  </button>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>

      {/* Queue Stats */}
      {files.length > 0 && (
        <div 
          className="mt-4 p-3 rounded-lg border animate-fade-in"
          style={{ backgroundColor: 'rgba(26, 26, 26, 0.8)', borderColor: '#333' }}
        >
          <div className="flex justify-between items-center text-sm">
            <div className="flex items-center gap-4">
              <span style={{ color: '#888' }}>
                Files in queue: <span className="text-white font-medium">{files.length}</span>
              </span>
              <span style={{ color: '#333' }}>|</span>
              <span className="flex items-center gap-2" style={{ color: '#888' }}>
                Output format:
                <span className={getFormatBadgeClass(outputFormat)}>
                  {getFormatIcon(outputFormat, "output")}
                  {outputFormat.toUpperCase()}
                </span>
              </span>
            </div>
            {isProOutput && (
              <span className="adobe-badge">
                <Clapperboard size={12} />
                Adobe Compatible
              </span>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
