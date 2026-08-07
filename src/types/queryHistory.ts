export interface QueryHistoryEntry {
  id: string;
  sql: string;
  executedAt: string;
  executionTimeMs: number | null;
  status: "success" | "error";
  rowsAffected: number | null;
  error: string | null;
  database: string | null;
  /**
   * Which connection ran this statement. Absent for the per-connection
   * queries, where it is implied; present when history is read across all
   * connections, since the rows are interleaved there.
   */
  connectionId?: string;
  connectionName?: string;
}

export interface QueryHistoryResponse {
  entries: QueryHistoryEntry[];
  recoveredBackupPath: string | null;
}

export interface QueryHistoryRecoveryNotice {
  connectionId: string;
  backupPath: string;
}
