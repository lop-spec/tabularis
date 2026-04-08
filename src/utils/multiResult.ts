import type { QueryResultEntry } from "../types/editor";

/**
 * Creates initial QueryResultEntry array from a list of queries.
 * All entries start in loading state.
 */
export function createResultEntries(
  tabId: string,
  queries: string[],
): QueryResultEntry[] {
  return queries.map((query, index) => ({
    id: `${tabId}-result-${index}`,
    queryIndex: index,
    query,
    result: null,
    error: "",
    executionTime: null,
    isLoading: true,
    page: 1,
    activeTable: null,
    pkColumn: null,
  }));
}

/**
 * Updates a single entry within a results array by id.
 * Returns a new array with the matching entry replaced.
 */
export function updateResultEntry(
  results: QueryResultEntry[],
  entryId: string,
  partial: Partial<QueryResultEntry>,
): QueryResultEntry[] {
  return results.map((r) =>
    r.id === entryId ? { ...r, ...partial } : r,
  );
}

/**
 * Finds the active result entry from a results array.
 * Falls back to the first entry if activeResultId is not found.
 */
export function findActiveEntry(
  results: QueryResultEntry[],
  activeResultId: string | undefined,
): QueryResultEntry | undefined {
  if (results.length === 0) return undefined;
  return results.find((r) => r.id === activeResultId) || results[0];
}

/**
 * Counts succeeded entries (not loading, no error, has result).
 */
export function countSucceeded(results: QueryResultEntry[]): number {
  return results.filter((r) => !r.isLoading && !r.error && r.result).length;
}

/**
 * Counts failed entries (not loading, has error).
 */
export function countFailed(results: QueryResultEntry[]): number {
  return results.filter((r) => !r.isLoading && r.error).length;
}

/**
 * Sums execution times across all entries.
 */
export function totalExecutionTime(results: QueryResultEntry[]): number {
  return results.reduce((sum, r) => sum + (r.executionTime ?? 0), 0);
}
