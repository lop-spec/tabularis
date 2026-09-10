import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { interruptBatchResults } from "../../src/utils/batchInterruption";
import { createResultEntries, totalExecutionTime } from "../../src/utils/multiResult";

describe("batch interruption reconciliation", () => {
  it("does not copy whole-batch time into thousands of unresolved statements", () => {
    const entries = createResultEntries("batch", Array.from({ length: 3436 }, (_, i) => `SELECT ${i}`));
    const confirmed = 1401;
    for (let i = 0; i < confirmed; i += 1) {
      entries[i] = {
        ...entries[i],
        isLoading: false,
        executionTime: 123,
        result: { columns: [], rows: [], affected_rows: 1, truncated: false },
      };
    }
    const resolved = interruptBatchResults(entries, "Query cancelled");
    expect(resolved).toHaveLength(3436);
    expect(resolved.every((entry) => !entry.isLoading)).toBe(true);
    for (let i = 0; i < confirmed; i += 1) expect(resolved[i]).toBe(entries[i]);
    expect(resolved.slice(confirmed).every((entry) => entry.executionTime === null)).toBe(true);
    expect(totalExecutionTime(resolved)).toBe(confirmed * 123);
    expect(entries[confirmed].isLoading).toBe(true);
  });

  it("preserves confirmed failures, skipped outcomes, and out-of-order progress", () => {
    const entries = createResultEntries("batch", ["SELECT 1", "SELECT 2", "SELECT 3"]);
    entries[1] = { ...entries[1], error: "Confirmed statement error", executionTime: 47, isLoading: false };
    entries[2] = { ...entries[2], error: "Skipped by choice", executionTime: 0, isLoading: false };
    const resolved = interruptBatchResults(entries, "Connection lost");
    expect(resolved[0]).toMatchObject({ error: "Connection lost", executionTime: null, isLoading: false });
    expect(resolved[1]).toBe(entries[1]);
    expect(resolved[2]).toBe(entries[2]);
    expect(interruptBatchResults(resolved, "Retry")).toEqual(resolved);
  });

  it("handles acquisition failures without suggesting any statement completed", () => {
    const entries = createResultEntries("batch", ["SELECT 1", "SELECT 2"]);
    const resolved = interruptBatchResults(entries, "Connection refused");
    expect(totalExecutionTime(resolved)).toBe(0);
    expect(resolved.every((entry) => entry.result === null && entry.executionTime === null)).toBe(true);
    expect(interruptBatchResults([], "Cancelled")).toEqual([]);
  });

  it("records a batch-level failure once, never each unresolved SQL as an execution", () => {
    const source = readFileSync("src/pages/Editor.tsx", "utf8");
    const failure = source.slice(source.indexOf("results: interruptBatchResults(batchEntries, message)"), source.indexOf("\n      unlisten();\n\n      // Reconcile"));
    expect(failure).toContain('entries.map((entry) => entry.query).join(";\\n")');
    expect(failure.match(/addHistoryEntry\(/g)).toHaveLength(1);
    expect(failure).not.toContain("fallbackElapsed");
    expect(source).toContain("batchEntries[index] = { ...batchEntries[index], ...partial }");
  });
});
