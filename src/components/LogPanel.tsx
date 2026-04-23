import { useEffect, useRef } from "react";
import { LogEntry } from "../hooks/useLogs";

interface LogPanelProps {
  logs: LogEntry[];
  onClose: () => void;
  onClear: () => void;
}

const LEVEL_STYLES: Record<string, string> = {
  ERROR: "text-red-500 bg-red-500/10",
  WARN: "text-yellow-500 bg-yellow-500/10",
  INFO: "text-blue-400 bg-blue-400/10",
  DEBUG: "text-gray-400 bg-gray-400/10",
  TRACE: "text-gray-500 bg-gray-500/10",
};

const LEVEL_BADGES: Record<string, string> = {
  ERROR: "bg-red-500/20 text-red-400",
  WARN: "bg-yellow-500/20 text-yellow-400",
  INFO: "bg-blue-500/20 text-blue-400",
  DEBUG: "bg-gray-500/20 text-gray-400",
  TRACE: "bg-gray-600/20 text-gray-500",
};

export function LogPanel({ logs, onClose, onClear }: LogPanelProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const autoScroll = useRef(true);

  useEffect(() => {
    if (autoScroll.current && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [logs]);

  const handleScroll = () => {
    if (!scrollRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = scrollRef.current;
    autoScroll.current = scrollHeight - scrollTop - clientHeight < 50;
  };

  return (
    <div className="fixed inset-0 z-50 flex items-end justify-center pointer-events-none">
      <div
        className="fixed inset-0 bg-black/40 pointer-events-auto"
        onClick={onClose}
      />
      <div className="relative w-full max-w-4xl h-[60vh] bg-gray-900 dark:bg-gray-950 border-t border-gray-700 rounded-t-2xl shadow-2xl pointer-events-auto flex flex-col animate-slide-up">
        <div className="flex items-center justify-between px-4 py-3 border-b border-gray-700/50">
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium text-gray-200">
              📋 Runtime Log
            </span>
            <span className="text-xs text-gray-500">
              {logs.length} entries
            </span>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={onClear}
              className="px-3 py-1 text-xs text-gray-400 hover:text-gray-200 bg-gray-800 hover:bg-gray-700 rounded-md transition-colors"
            >
              Clear
            </button>
            <button
              onClick={() => {
                autoScroll.current = true;
                if (scrollRef.current) {
                  scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
                }
              }}
              className="px-3 py-1 text-xs text-gray-400 hover:text-gray-200 bg-gray-800 hover:bg-gray-700 rounded-md transition-colors"
            >
              ↓ Scroll to Bottom
            </button>
            <button
              onClick={onClose}
              className="px-3 py-1 text-xs text-gray-400 hover:text-red-400 bg-gray-800 hover:bg-gray-700 rounded-md transition-colors"
            >
              ✕ Close
            </button>
          </div>
        </div>

        <div
          ref={scrollRef}
          onScroll={handleScroll}
          className="flex-1 overflow-y-auto px-4 py-2 font-mono text-xs leading-5"
        >
          {logs.length === 0 ? (
            <div className="text-gray-500 text-center py-8">
              Waiting for log output...
            </div>
          ) : (
            logs.map((log, i) => (
              <div
                key={i}
                className={`flex gap-2 py-0.5 hover:bg-white/5 rounded px-1 ${
                  log.level === "ERROR" ? "bg-red-500/5" : ""
                } ${log.level === "WARN" ? "bg-yellow-500/5" : ""}`}
              >
                <span className="text-gray-600 shrink-0 w-20">
                  {log.timestamp}
                </span>
                <span
                  className={`shrink-0 w-12 text-center rounded px-1 py-0 text-[10px] font-bold ${
                    LEVEL_BADGES[log.level] || "bg-gray-700 text-gray-400"
                  }`}
                >
                  {log.level}
                </span>
                <span
                  className={`shrink-0 w-40 truncate ${
                    LEVEL_STYLES[log.level] || "text-gray-400"
                  }`}
                  title={log.target}
                >
                  {log.target.split("::").slice(-2).join("::")}
                </span>
                <span className="text-gray-300 break-all">{log.message}</span>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
