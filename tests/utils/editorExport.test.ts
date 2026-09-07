import { readFileSync } from "node:fs";
import ts from "typescript";
import { describe, expect, it, vi } from "vitest";
import { getExportableResultEntries, buildBatchExportFileName } from "../../src/utils/batchExport";
import { formatResultForExport, getLoadedRowsExportLimit } from "../../src/utils/resultExport";

// Exercise the real event handler with inert desktop boundaries, without mounting
// Monaco and every database provider. AST extraction fails if the handler moves.
const source = ts.createSourceFile("Editor.tsx", readFileSync("src/pages/Editor.tsx", "utf8"), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
let handler = "";
function visit(node: ts.Node) {
  if (ts.isVariableDeclaration(node) && node.name.getText(source) === "handleExportCommon" && node.initializer) {
    handler = node.initializer.getText(source);
  }
  ts.forEachChild(node, visit);
}
visit(source);
if (!handler) throw new Error("Export handler not found");
const compiled = ts.transpile(`const run = ${handler};`, { target: ts.ScriptTarget.ES2022 });

function harness(activeTab: Record<string, unknown>) {
  const invoke = vi.fn().mockResolvedValue(undefined);
  const writeTextFile = vi.fn().mockResolvedValue(undefined);
  let state: Record<string, unknown> = {};
  const scope = {
    activeTab, activeConnectionId: "inert-connection", activeCapabilities: { schemas: false }, activeDriver: "mysql",
    exportCancelledRef: { current: false }, localExportRef: { current: false },
    csvDelimiter: ",", getExportableResultEntries, buildBatchExportFileName, formatResultForExport, getLoadedRowsExportLimit,
    t: (key: string) => key, invoke, writeTextFile,
    save: vi.fn().mockResolvedValue("/output/result.csv"), open: vi.fn().mockResolvedValue("/output"),
    join: async (...paths: string[]) => paths.join("/"),
    getExecutionScopeForTab: () => "fixture", isMultiDatabaseCapable: () => true,
    reconstructTableQuery: () => "SELECT * FROM `fixture`.`rows`",
    setExportMenuOpen: vi.fn(),
    setExportState: (next: Record<string, unknown> | ((prev: Record<string, unknown>) => Record<string, unknown>)) => {
      state = typeof next === "function" ? next(state) : next;
    },
  };
  const run = new Function(...Object.keys(scope), `${compiled}; return run;`)(...Object.values(scope)) as (format: string) => Promise<void>;
  return { run, invoke, writeTextFile, state: () => state, cancel: () => { scope.exportCancelledRef.current = true; } };
}
const result = { columns: ["id"], rows: [[1]], affected_rows: 1 };
const entry = (query: string, index = 0) => ({ id: String(index), queryIndex: index, query, result });

describe("editor export does not replay console SQL", () => {
  it("exports RETURNING results without invoking a query", async () => {
    const h = harness({ type: "console", results: [entry("DELETE FROM rows RETURNING id")] });
    await h.run("csv");
    expect(h.invoke).not.toHaveBeenCalled();
    expect(h.writeTextFile).toHaveBeenCalledWith("/output/result.csv", "id\n1");
    expect(h.state().status).toBe("completed");
  });
  it("preserves multi-file export without re-executing any statement", async () => {
    const h = harness({ type: "console", results: [entry("CALL modifies_data()"), entry("SELECT 1", 1)] });
    await h.run("csv");
    expect(h.invoke).not.toHaveBeenCalled();
    expect(h.writeTextFile).toHaveBeenCalledTimes(2);
    expect(h.writeTextFile.mock.calls.map(([path]) => path)).toEqual(["/output/01_result_1.csv", "/output/02_result_2.csv"]);
  });
  it("exports an empty result's headers instead of replaying SQL", async () => {
    const h = harness({ type: "console", results: [{ ...entry("DELETE FROM rows RETURNING id"), result: { ...result, rows: [] } }] });
    await h.run("csv");
    expect(h.invoke).not.toHaveBeenCalled();
    expect(h.writeTextFile).toHaveBeenCalledWith("/output/result.csv", "id");
  });
  it("keeps a truncation warning after a later complete result", async () => {
    const log = vi.spyOn(console, "warn").mockImplementation(() => {});
    const h = harness({ type: "console", results: [
      { ...entry("SELECT 1"), result: { ...result, pagination: { total_rows: 100 } } }, entry("SELECT 2", 1),
    ] });
    await h.run("csv");
    expect(h.state().warningMessage).toBe("editor.exportLoadedRowsWarning");
    expect(log).toHaveBeenCalledOnce();
    log.mockRestore();
  });
  it("stops a cancelled batch before writing the next file", async () => {
    const h = harness({ type: "console", results: [entry("SELECT 1"), entry("SELECT 2", 1)] });
    h.writeTextFile.mockImplementationOnce(async () => { h.cancel(); });
    await h.run("csv");
    expect(h.writeTextFile).toHaveBeenCalledTimes(1);
    expect(h.state().status).not.toBe("completed");
  });
  it("keeps table full-export available", async () => {
    const h = harness({ type: "table", activeTable: "rows", query: "", result });
    await h.run("csv");
    expect(h.invoke).toHaveBeenCalledWith("export_query_to_file", expect.objectContaining({ query: "SELECT * FROM `fixture`.`rows`" }));
  });
  it("refuses to re-run a console with no result", async () => {
    const log = vi.spyOn(console, "warn").mockImplementation(() => {});
    const h = harness({ type: "console", query: "DELETE FROM rows" });
    await h.run("csv");
    expect(h.invoke).not.toHaveBeenCalled();
    expect(h.writeTextFile).not.toHaveBeenCalled();
    expect(log).toHaveBeenCalledOnce();
    log.mockRestore();
  });
});
