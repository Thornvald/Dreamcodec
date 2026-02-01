import { useRef, useEffect } from "react";
import { Terminal } from "lucide-react";

interface LogOutputProps {
  logs: string[];
}

export default function LogOutput({ logs }: LogOutputProps) {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [logs]);

  return (
    <div className="flex flex-col h-full p-4">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <Terminal size={18} style={{ color: '#888' }} />
          <h3 className="font-medium" style={{ color: '#e0e0e0' }}>Conversion Logs</h3>
        </div>
      </div>

      <div
        ref={scrollRef}
        className="flex-1 rounded-lg border p-4 font-mono text-xs overflow-y-auto"
        style={{ backgroundColor: '#0a0a0a', borderColor: '#222' }}
      >
        {logs.length === 0 ? (
          <p className="italic" style={{ color: '#444' }}>No logs yet...</p>
        ) : (
          <div className="space-y-1">
            {logs.map((log, index) => (
              <p
                key={index}
                style={{
                  color: log.includes("Error") || log.includes("Failed")
                    ? '#f87171'
                    : log.includes("Completed") || log.includes("Success")
                    ? '#4ade80'
                    : log.includes("Started") || log.includes("Converting")
                    ? '#888'
                    : '#888'
                }}
              >
                {log}
              </p>
            ))}
          </div>
        )}
      </div>

      <div className="mt-2 text-xs text-right" style={{ color: '#555' }}>
        {logs.length} log entries
      </div>
    </div>
  );
}
