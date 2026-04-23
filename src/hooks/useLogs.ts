import { listen } from "@tauri-apps/api/event";
import { useState, useCallback, useEffect, useRef } from "react";

export interface LogEntry {
  timestamp: string;
  level: string;
  target: string;
  message: string;
}

const MAX_LOG_ENTRIES = 2000;

export function useLogs() {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [isOpen, setIsOpen] = useState(false);
  const listenersReady = useRef(false);

  useEffect(() => {
    if (listenersReady.current) return;
    listenersReady.current = true;

    const unlisten = listen<LogEntry>("log-entry", (event) => {
      setLogs((prev) => {
        const next = [...prev, event.payload];
        return next.length > MAX_LOG_ENTRIES ? next.slice(-MAX_LOG_ENTRIES) : next;
      });
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const toggleOpen = useCallback(() => {
    setIsOpen((prev) => !prev);
  }, []);

  const clearLogs = useCallback(() => {
    setLogs([]);
  }, []);

  return { logs, isOpen, toggleOpen, clearLogs };
}
