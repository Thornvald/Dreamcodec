import { CheckCircle, XCircle, Loader2, X, Trash2, Film, Music, Clapperboard } from "lucide-react";
import type { ConversionTask } from "../types";

interface ConversionProgressProps {
  conversions: ConversionTask[];
  onCancel: (taskId: string) => void;
  onClearCompleted: () => void;
}

export default function ConversionProgress({
  conversions,
  onCancel,
  onClearCompleted,
}: ConversionProgressProps) {
  const getFileName = (path: string) => {
    return path.split("/").pop() || path.split("\\").pop() || path;
  };

  const getStatusIcon = (status: ConversionTask["status"]) => {
    switch (status) {
      case "converting":
        return <Loader2 size={18} className="animate-spin" style={{ color: '#888' }} />;
      case "completed":
        return <CheckCircle size={18} style={{ color: '#22c55e' }} />;
      case "failed":
        return <XCircle size={18} style={{ color: '#ef4444' }} />;
      case "cancelled":
        return <X size={18} style={{ color: '#555' }} />;
      default:
        return <Loader2 size={18} style={{ color: '#555' }} />;
    }
  };

  const getStatusText = (status: ConversionTask["status"]) => {
    switch (status) {
      case "converting":
        return "Converting...";
      case "completed":
        return "Completed";
      case "failed":
        return "Failed";
      case "cancelled":
        return "Cancelled";
      default:
        return "Pending";
    }
  };

  const getStatusColor = (status: ConversionTask["status"]) => {
    switch (status) {
      case "converting":
        return "text-gray-400";
      case "completed":
        return "text-green-500";
      case "failed":
        return "text-red-500";
      case "cancelled":
        return "text-gray-600";
      default:
        return "text-gray-500";
    }
  };

  const getFormatIcon = (format: string) => {
    const isAudio = ["mp3", "wav", "aac", "flac", "m4a"].includes(format);
    const isPro = ["prores", "dnxhd"].includes(format);
    
    if (isAudio) {
      return <Music size={14} style={{ color: '#aaa' }} />;
    }
    if (isPro) {
      return <Clapperboard size={14} style={{ color: '#ccc' }} />;
    }
    return <Film size={14} style={{ color: '#888' }} />;
  };

  const getFormatBadgeClass = (format: string) => {
    const isAudio = ["mp3", "wav", "aac", "flac", "m4a"].includes(format);
    const isPro = ["prores", "dnxhd"].includes(format);
    
    if (isAudio) return "format-icon format-audio";
    if (isPro) return "format-icon format-pro";
    return "format-icon format-video";
  };

  const completedCount = conversions.filter(
    c => c.status === "completed" || c.status === "failed" || c.status === "cancelled"
  ).length;

  const activeCount = conversions.filter(c => c.status === "converting").length;

  return (
    <div className="flex flex-col h-full p-4">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <div>
          <h3 className="font-medium" style={{ color: '#e0e0e0' }}>Conversion Progress</h3>
          <p className="text-sm" style={{ color: '#888' }}>
            <span style={{ color: '#888' }}>{activeCount}</span> active,{" "}
            <span style={{ color: '#22c55e' }}>{completedCount}</span> completed{" "}
            <span style={{ color: '#555' }}>/ {conversions.length} total</span>
          </p>
        </div>
        {completedCount > 0 && (
          <button
            onClick={onClearCompleted}
            className="btn btn-secondary flex items-center gap-2 text-sm"
          >
            <Trash2 size={16} />
            Clear Completed
          </button>
        )}
      </div>

      {/* Conversions List */}
      <div className="flex-1 overflow-y-auto">
        {conversions.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full" style={{ color: '#555' }}>
            <p>No active conversions</p>
            <p className="text-sm">Add files to the queue and start conversion</p>
          </div>
        ) : (
          <div className="space-y-3">
            {conversions.map((conversion, index) => (
              <div
                key={conversion.id}
                className="card card-glow animate-fade-in"
                style={{ 
                  animationDelay: `${index * 100}ms`,
                  backgroundColor: '#1a1a1a',
                  borderColor: '#333'
                }}
              >
                {/* File Info */}
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-2 min-w-0">
                    {getStatusIcon(conversion.status)}
                    <span className="font-medium truncate" style={{ color: '#e0e0e0' }}>
                      {getFileName(conversion.inputFile)}
                    </span>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className={`text-sm ${getStatusColor(conversion.status)}`}>
                      {getStatusText(conversion.status)}
                    </span>
                    {conversion.status === "converting" && (
                      <button
                        onClick={() => onCancel(conversion.id)}
                        className="p-1 rounded transition-colors hover:bg-red-500/10"
                        style={{ color: '#888' }}
                        onMouseEnter={(e) => e.currentTarget.style.color = '#f87171'}
                        onMouseLeave={(e) => e.currentTarget.style.color = '#888'}
                      >
                        <X size={16} />
                      </button>
                    )}
                  </div>
                </div>

                {/* Format Info */}
                <div className="flex items-center gap-2 mb-2 text-xs">
                  <span className="flex items-center gap-1" style={{ color: '#555' }}>
                    {conversion.inputFormat && (
                      <>
                        <span className="uppercase">{conversion.inputFormat}</span>
                      </>
                    )}
                  </span>
                  <span style={{ color: '#444' }}>→</span>
                  <span className={`flex items-center gap-1 ${getFormatBadgeClass(conversion.outputFormat)}`}>
                    {getFormatIcon(conversion.outputFormat)}
                    <span className="uppercase">{conversion.outputFormat}</span>
                  </span>
                </div>

                {/* Progress Bar */}
                <div className="mb-2">
                  <div className="progress-bar">
                    <div
                      className="progress-bar-fill"
                      style={{ width: `${conversion.progress}%` }}
                    />
                  </div>
                </div>

                {/* Progress Stats */}
                <div className="flex justify-between text-xs" style={{ color: '#888' }}>
                  <span style={{ color: conversion.progress > 0 ? '#aaa' : '#888' }}>
                    {conversion.progress.toFixed(1)}%
                  </span>
                  <span className="truncate max-w-[50%]">{getFileName(conversion.outputFile)}</span>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
