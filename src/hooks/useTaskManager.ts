import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { SystemStats } from "../utils/taskManager";

const POLL_INTERVAL_MS = 2000;

interface UseTaskManagerResult {
  systemStats: SystemStats | null;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

export function useTaskManager(): UseTaskManagerResult {
  const [systemStats, setSystemStats] = useState<SystemStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchData = useCallback(async () => {
    try {
      setSystemStats(await invoke<SystemStats>("get_system_stats"));
      setError(null);
    } catch (value) {
      setError(value instanceof Error ? value.message : String(value));
    } finally {
      setLoading(false);
    }
  }, []);

  const refresh = useCallback(async () => {
    setLoading(true);
    await fetchData();
  }, [fetchData]);

  useEffect(() => {
    void fetchData();
    intervalRef.current = setInterval(fetchData, POLL_INTERVAL_MS);
    return () => {
      if (intervalRef.current !== null) clearInterval(intervalRef.current);
    };
  }, [fetchData]);

  return { systemStats, loading, error, refresh };
}
