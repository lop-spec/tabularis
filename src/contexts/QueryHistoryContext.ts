import { createContext } from "react";
import type { QueryHistoryEntry } from "../types/queryHistory";

export type { QueryHistoryEntry };

export interface QueryHistoryContextType {
  entries: QueryHistoryEntry[];
  isLoading: boolean;
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
}

export const QueryHistoryContext = createContext<
  QueryHistoryContextType | undefined
>(undefined);
