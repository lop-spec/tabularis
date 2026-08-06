import { describe, it, expect } from "vitest";
import { requiresRollbackProtectedExecution } from "../../src/utils/rollbackProtection";

// The guard receives the whole editor selection, not one statement at a time
// (Editor.tsx hands `[textToRun]` to the runner and lets the backend split it).
// A batch must therefore be protected when *any* statement in it needs it —
// judging by the leading keyword alone lets a write ride in behind a read.
describe("multi-statement batches", () => {
  it("protects a batch whose first statement is a session variable", () => {
    expect(
      requiresRollbackProtectedExecution(
        "SET @cutoff = '2026-01-01';\nDELETE FROM biz_order_record WHERE created_at < @cutoff;",
      ),
    ).toBe(true);
  });

  it("protects a batch whose first statement is a SELECT", () => {
    expect(
      requiresRollbackProtectedExecution(
        "SELECT COUNT(*) FROM t;\nUPDATE t SET status = 1 WHERE id = 2;",
      ),
    ).toBe(true);
  });

  it("protects a write hidden behind a comment-only prelude", () => {
    expect(
      requiresRollbackProtectedExecution(
        "-- pre-flight\nSELECT 1;\nDROP TABLE staging_tmp;",
      ),
    ).toBe(true);
  });

  it("still allows lop's read-only pre-flight batch", () => {
    expect(
      requiresRollbackProtectedExecution(
        [
          "SET @batch_started_at = CURRENT_TIMESTAMP;",
          "SET @expected_rows = 8;",
          "SELECT COUNT(*) AS total FROM biz_kb_qa_content WHERE id IN (1, 2, 3);",
          "SELECT id, content FROM biz_kb_qa_content WHERE id IN (1, 2, 3);",
        ].join("\n"),
      ),
    ).toBe(false);
  });

  it("still allows a plain multi-SELECT batch", () => {
    expect(
      requiresRollbackProtectedExecution("SELECT 1;\nSELECT 2;\nSHOW TABLES;"),
    ).toBe(false);
  });

  it("protects a batch where only a later statement writes a file", () => {
    expect(
      requiresRollbackProtectedExecution(
        "SELECT 1;\nSELECT * FROM t INTO OUTFILE '/tmp/x.csv';",
      ),
    ).toBe(true);
  });
});
