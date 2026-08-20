import { describe, expect, it } from "vitest";
import {
  createRollbackViewerTab,
  findRollbackFile,
} from "../../src/utils/rollbackFile";

describe("rollbackFile", () => {
  it("returns the first real rollback file from a protected batch", () => {
    expect(
      findRollbackFile([
        {
          result: null,
          error: null,
          execution_time_ms: 4,
          rollback_file: "",
        },
        {
          result: null,
          error: null,
          execution_time_ms: 8,
          rollback_file: "C:/Tabularis/rollback-sql/run.rollback.sql",
        },
      ]),
    ).toBe("C:/Tabularis/rollback-sql/run.rollback.sql");
  });

  it("does not expose an entry when no rollback file was generated", () => {
    expect(
      findRollbackFile([
        {
          result: null,
          error: null,
          execution_time_ms: 2,
        },
      ]),
    ).toBeUndefined();
  });

  it("creates a read-only rollback.sql viewer in the current database scope", () => {
    expect(createRollbackViewerTab("-- rollback\nUPDATE users SET active = 1;", "app")).toEqual({
      type: "console",
      title: "rollback.sql",
      query: "-- rollback\nUPDATE users SET active = 1;",
      readOnly: true,
      schema: "app",
    });
  });
});
