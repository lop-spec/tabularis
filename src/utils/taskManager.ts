export interface TabularisChildProcess {
  pid: number;
  name: string;
  cpu_percent: number;
  memory_bytes: number;
}

export interface TabularisSelfStats {
  pid: number;
  cpu_percent: number;
  self_memory_bytes: number;
  total_memory_bytes: number;
  disk_read_bytes: number;
  disk_write_bytes: number;
  child_count: number;
}

export interface SystemStats {
  cpu_percent: number;
  memory_used: number;
  memory_total: number;
  disk_read_bytes: number;
  disk_write_bytes: number;
  process_count: number;
  tabularis: TabularisSelfStats | null;
}

const BYTES_IN_KB = 1024;
const BYTES_IN_MB = 1024 * 1024;
const BYTES_IN_GB = 1024 * 1024 * 1024;

export function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
  if (bytes < BYTES_IN_KB) return `${bytes} B`;
  if (bytes < BYTES_IN_MB) return `${(bytes / BYTES_IN_KB).toFixed(1)} KB`;
  if (bytes < BYTES_IN_GB) return `${(bytes / BYTES_IN_MB).toFixed(1)} MB`;
  return `${(bytes / BYTES_IN_GB).toFixed(2)} GB`;
}

export function formatCpuPercent(percent: number): string {
  return `${Math.max(0, Math.min(100, percent)).toFixed(1)}%`;
}

export function formatMemoryBar(used: number, total: number): number {
  if (total <= 0) return 0;
  return Math.min(100, Math.round((used / total) * 100));
}
