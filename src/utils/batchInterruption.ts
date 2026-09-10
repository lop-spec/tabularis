import type { QueryResultEntry } from "../types/editor";

/**
 * A batch-level failure has no per-statement timing for unresolved slots.
 * Preserve confirmed outcomes and resolve the rest in one linear update.
 * They may include both an interrupted statement and statements never sent;
 * neither should be fabricated as an individually executed history entry.
 */
export function interruptBatchResults(
  results: QueryResultEntry[],
  reason: string,
): QueryResultEntry[] {
  return results.map((entry) => entry.isLoading ? {
    ...entry,
    error: reason,
    executionTime: null,
    isLoading: false,
  } : entry);
}
