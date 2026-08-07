import { createContext } from "react";
import type {
  QueryHistoryEntry,
  QueryHistoryRecoveryNotice,
} from "../types/queryHistory";

export type { QueryHistoryEntry };

export interface QueryHistoryContextType {
  entries: QueryHistoryEntry[];
  isLoading: boolean;
  recoveryNotice: QueryHistoryRecoveryNotice | null;
  dismissRecoveryNotice: () => void;
  addEntry: (
    sql: string,
    executionTimeMs: number | null,
    status: "success" | "error",
    rowsAffected: number | null,
    error: string | null,
    database?: string | null,
  ) => Promise<void>;
  deleteEntry: (id: string) => Promise<void>;
  clearHistory: () => Promise<void>;
  refreshHistory: () => Promise<void>;
  /** Searches the full on-disk log, not just the loaded page. */
  searchHistory: (query: string, limit?: number) => Promise<QueryHistoryEntry[]>;
  /**
   * Newest entries across every connection, not just the active one.
   *
   * The sidebar is scoped to the current connection because it is contextual;
   * the history page is not, and requiring a selection there just hides
   * statements the user is looking for.
   */
  loadAllHistory: (limit?: number) => Promise<QueryHistoryEntry[]>;
  /** Same scope as {@link loadAllHistory}, filtered by substring. */
  searchAllHistory: (
    query: string,
    limit?: number,
  ) => Promise<QueryHistoryEntry[]>;
}

export const QueryHistoryContext = createContext<
  QueryHistoryContextType | undefined
>(undefined);
