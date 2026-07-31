import { describe, expect, it } from "vitest";
import {
  ROLLBACK_RISK_REVIEW_PREFIX,
  parseRollbackRiskReview,
  requiresRollbackProtectedExecution,
} from "../../src/utils/rollbackProtection";

describe("requiresRollbackProtectedExecution", () => {
  it("keeps clearly read-only statements on the normal single-query path", () => {
    for (const sql of [
      "SELECT * FROM users",
      "-- header\nSHOW TABLES",
      "/* header */ EXPLAIN SELECT * FROM users",
      "DESCRIBE users",
      "VALUES ROW(1)",
      "",
      "-- comment only",
    ]) {
      expect(requiresRollbackProtectedExecution(sql)).toBe(false);
    }
  });

  it("routes writes, DDL, session state, CTEs, and unknown families through the backend planner", () => {
    for (const sql of [
      "UPDATE users SET active = 0 WHERE id = 1",
      "INSERT INTO users (id) VALUES (1)",
      "DELETE FROM users WHERE id = 1",
      "CREATE TABLE users_archive (id bigint)",
      "TRUNCATE TABLE users",
      "SET sql_safe_updates = 1",
      "USE reporting",
      "CALL refresh_users()",
      "WITH changed AS (SELECT 1) UPDATE users SET active = 1",
      "/*!40101 SET @OLD_SQL_MODE=@@SQL_MODE */",
      "VENDOR_WRITE users",
    ]) {
      expect(requiresRollbackProtectedExecution(sql)).toBe(true);
    }
  });
});

describe("parseRollbackRiskReview", () => {
  it("parses a backend review containing every unsupported statement", () => {
    const payload = {
      statements: [
        {
          index: 2,
          sql: "INSERT INTO users (id) SELECT id FROM staging",
          reason: "INSERT SELECT cannot be rolled back exactly",
          destructive: false,
        },
        {
          index: 4,
          sql: "TRUNCATE TABLE users",
          reason: "deleted rows cannot be reconstructed",
          destructive: true,
        },
      ],
    };

    expect(
      parseRollbackRiskReview(
        `${ROLLBACK_RISK_REVIEW_PREFIX}${JSON.stringify(payload)}`,
      ),
    ).toEqual({ ...payload, kind: "unsupported" });
  });

  it("rejects unrelated, malformed, or structurally invalid errors", () => {
    expect(parseRollbackRiskReview("connection failed")).toBeNull();
    expect(
      parseRollbackRiskReview(`${ROLLBACK_RISK_REVIEW_PREFIX}{bad json`),
    ).toBeNull();
    expect(
      parseRollbackRiskReview(
        `${ROLLBACK_RISK_REVIEW_PREFIX}${JSON.stringify({
          statements: [{ index: 0, sql: "", reason: "" }],
        })}`,
      ),
    ).toBeNull();
  });
});
