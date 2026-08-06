import { describe, it, expect } from "vitest";
import { requiresRollbackProtectedExecution } from "../../src/utils/rollbackProtection";

// lop's batch: session variables used as a read-only pre-flight check.
// `SET @var` touches no data and cannot need a rollback, but it used to drag
// the whole batch (both SELECTs included) into the protected path.
describe("session variable assignments", () => {
  it("does not require rollback protection", () => {
    for (const sql of [
      "SET @batch_started_at = CURRENT_TIMESTAMP",
      "SET @expected_rows = 8;",
      "SET @expected_content_bad = 8",
      "set  @x := 1",
      "SET @a = 1, @b = 2",
    ]) {
      expect(requiresRollbackProtectedExecution(sql)).toBe(false);
    }
  });

  it("still protects SET forms that change server or session behaviour", () => {
    for (const sql of [
      "SET GLOBAL read_only = 1",
      "SET SESSION sql_safe_updates = 0",
      "SET sql_safe_updates = 1",
      "SET autocommit = 0",
      "SET NAMES utf8mb4",
      "SET TRANSACTION ISOLATION LEVEL READ COMMITTED",
      "SET PERSIST max_connections = 500",
    ]) {
      expect(requiresRollbackProtectedExecution(sql)).toBe(true);
    }
  });
});
