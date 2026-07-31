import { describe, expect, it } from "vitest";
import {
  buildBatchExportFileName,
  getExportableResultEntries,
} from "../../src/utils/batchExport";
import type { QueryResultEntry } from "../../src/types/editor";

function entry(
  id: string,
  overrides: Partial<QueryResultEntry> = {},
): QueryResultEntry {
  return {
    id,
    queryIndex: 0,
    query: "SELECT 1",
    result: { columns: ["id"], rows: [[1]], affected_rows: 0 },
    error: "",
    executionTime: 1,
    isLoading: false,
    page: 1,
    activeTable: null,
    pkColumns: null,
    ...overrides,
  };
}

describe("batch export", () => {
  it("keeps only completed query result sets with columns", () => {
    const results = [
      entry("ok"),
      entry("loading", { isLoading: true }),
      entry("error", { error: "failed", result: null }),
      entry("affected", {
        result: { columns: [], rows: [], affected_rows: 3 },
      }),
    ];

    expect(getExportableResultEntries(results).map((result) => result.id)).toEqual([
      "ok",
    ]);
  });

  it("creates stable indexed filenames and removes Windows-invalid characters", () => {
    const result = entry("named", {
      queryIndex: 3,
      label: 'Revenue: Q3 / "final"',
    });

    expect(buildBatchExportFileName(result, 1, "csv")).toBe(
      "02_Revenue_ Q3 _ _final_.csv",
    );
  });

  it("uses the query index when no custom label exists", () => {
    expect(
      buildBatchExportFileName(entry("fallback", { queryIndex: 4 }), 0, "json"),
    ).toBe("01_result_5.json");
  });
});
