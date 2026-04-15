export interface QueryHistoryEntry {
  id: string;
  sql: string;
  executedAt: string;
  executionTimeMs: number | null;
  status: "success" | "error";
  rowsAffected: number | null;
  error: string | null;
}
