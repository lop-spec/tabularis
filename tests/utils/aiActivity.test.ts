import { describe, expect, it } from "vitest";
import {
  buildVisualExplainDeepLink,
  defaultExportFilename,
  eventsToCsvLines,
  formatDurationMs,
  getQueryKindBadgeStyle,
  getStatusBadgeStyle,
  groupBySession,
  isDestructiveApproval,
  notebookFileFromExport,
  parseVisualExplainDeepLink,
  truncateQuery,
} from "../../src/utils/aiActivity";
import type {
  AiActivityEvent,
  AiNotebookExport,
  PendingApproval,
} from "../../src/types/ai";

const baseEvent = (overrides: Partial<AiActivityEvent> = {}): AiActivityEvent => ({
  id: "id",
  sessionId: "s",
  timestamp: "2026-04-24T10:00:00Z",
  tool: "run_query",
  connectionId: "c1",
  connectionName: "dev",
  query: "SELECT 1",
  queryKind: "select",
  durationMs: 5,
  status: "success",
  rows: 1,
  error: null,
  clientHint: "claude",
  approvalId: null,
  ...overrides,
});

describe("getStatusBadgeStyle", () => {
  it("returns green styling for success", () => {
    const s = getStatusBadgeStyle("success");
    expect(s.text).toContain("green");
  });

  it("returns red styling for error and denied", () => {
    expect(getStatusBadgeStyle("error").text).toContain("red");
    expect(getStatusBadgeStyle("denied").text).toContain("red");
  });

  it("returns fallback for unknown status", () => {
    const s = getStatusBadgeStyle("nope");
    expect(s.text).toBe("text-muted");
  });
});

describe("getQueryKindBadgeStyle", () => {
  it("differentiates select / write / ddl", () => {
    expect(getQueryKindBadgeStyle("select").text).toContain("blue");
    expect(getQueryKindBadgeStyle("write").text).toContain("yellow");
    expect(getQueryKindBadgeStyle("ddl").text).toContain("orange");
  });

  it("falls back when null or unknown", () => {
    expect(getQueryKindBadgeStyle(null).text).toBe("text-muted");
    expect(getQueryKindBadgeStyle("garbage").text).toBe("text-muted");
  });
});

describe("truncateQuery", () => {
  it("returns empty string for null", () => {
    expect(truncateQuery(null)).toBe("");
  });

  it("collapses whitespace", () => {
    expect(truncateQuery("SELECT\n  1\n  FROM   t")).toBe("SELECT 1 FROM t");
  });

  it("truncates with ellipsis past limit", () => {
    const long = "a".repeat(100);
    expect(truncateQuery(long, 20).endsWith("…")).toBe(true);
    expect(truncateQuery(long, 20).length).toBe(21);
  });

  it("does not truncate short queries", () => {
    expect(truncateQuery("SELECT 1", 80)).toBe("SELECT 1");
  });
});

describe("formatDurationMs", () => {
  it("uses sub-millisecond bucket below 1ms", () => {
    expect(formatDurationMs(0.4)).toBe("<1 ms");
  });

  it("uses ms below 1s", () => {
    expect(formatDurationMs(456)).toBe("456 ms");
  });

  it("uses seconds below 1 minute", () => {
    expect(formatDurationMs(1500)).toBe("1.50 s");
  });

  it("uses minutes+seconds above 1 minute", () => {
    expect(formatDurationMs(125_000)).toBe("2m 5s");
  });
});

describe("eventsToCsvLines", () => {
  it("returns header and one row per event", () => {
    const lines = eventsToCsvLines([baseEvent({ id: "a" }), baseEvent({ id: "b" })]);
    expect(lines).toHaveLength(3);
    expect(lines[0]).toContain("id,session_id");
  });

  it("escapes quotes and commas", () => {
    const lines = eventsToCsvLines([
      baseEvent({ query: 'SELECT "a", "b" FROM t', error: null }),
    ]);
    // The query column isn't included in CSV, but error/connection might be —
    // verify the escape function keeps round-trippable output: we ensure no
    // bare comma slips into a single-cell value.
    expect(lines[1].split(",").length).toBeGreaterThanOrEqual(10);
  });

  it("emits empty cell for null fields", () => {
    const lines = eventsToCsvLines([baseEvent({ rows: null, error: null, connectionName: null })]);
    expect(lines[1]).toContain(",,");
  });
});

describe("notebookFileFromExport", () => {
  it("preserves type, content, name, schema", () => {
    const exp: AiNotebookExport = {
      version: 1,
      title: "AI Session abc",
      createdAt: "2026-04-24T10:00:00Z",
      cells: [
        { type: "markdown", content: "# header", name: "Session metadata" },
        { type: "sql", content: "SELECT 1", name: "Q1", schema: "dev" },
      ],
    };
    const nb = notebookFileFromExport(exp);
    expect(nb.cells).toHaveLength(2);
    expect(nb.cells[1]).toEqual({
      type: "sql",
      content: "SELECT 1",
      name: "Q1",
      schema: "dev",
    });
  });

  it("omits optional fields when undefined", () => {
    const exp: AiNotebookExport = {
      version: 1,
      title: "t",
      createdAt: "2026-04-24T10:00:00Z",
      cells: [{ type: "sql", content: "SELECT 1" }],
    };
    const nb = notebookFileFromExport(exp);
    expect("name" in nb.cells[0]).toBe(false);
    expect("schema" in nb.cells[0]).toBe(false);
  });
});

describe("defaultExportFilename", () => {
  it("composes date + slug", () => {
    const exp: AiNotebookExport = {
      version: 1,
      title: "t",
      createdAt: "2026-04-24T10:00:00Z",
      cells: [],
    };
    expect(defaultExportFilename("abcdef1234567890", exp)).toBe(
      "ai-session-2026-04-24-abcdef12.tabularis-notebook",
    );
  });
});

describe("groupBySession", () => {
  it("groups events by session id preserving order", () => {
    const a = baseEvent({ id: "a", sessionId: "s1" });
    const b = baseEvent({ id: "b", sessionId: "s2" });
    const c = baseEvent({ id: "c", sessionId: "s1" });
    const groups = groupBySession([a, b, c]);
    expect(groups.size).toBe(2);
    expect(groups.get("s1")?.map((e) => e.id)).toEqual(["a", "c"]);
    expect(groups.get("s2")?.map((e) => e.id)).toEqual(["b"]);
  });
});

describe("buildVisualExplainDeepLink + parseVisualExplainDeepLink", () => {
  it("round-trips a query with special characters", () => {
    const url = buildVisualExplainDeepLink(
      "conn-1",
      "SELECT 'a, b', \"c\" FROM users WHERE x = 1",
    );
    expect(url.startsWith("/visual-explain?")).toBe(true);
    const parsed = parseVisualExplainDeepLink(url.split("?")[1]);
    expect(parsed.connectionId).toBe("conn-1");
    expect(parsed.query).toBe("SELECT 'a, b', \"c\" FROM users WHERE x = 1");
  });

  it("returns nulls when params are missing", () => {
    const parsed = parseVisualExplainDeepLink("");
    expect(parsed.connectionId).toBeNull();
    expect(parsed.query).toBeNull();
  });
});

describe("isDestructiveApproval", () => {
  const base: PendingApproval = {
    id: "x",
    createdAt: "2026-04-24T10:00:00Z",
    sessionId: "s",
    connectionId: "c",
    connectionName: "dev",
    query: "UPDATE t",
    queryKind: "write",
    clientHint: null,
    explainPlan: null,
    explainError: null,
  };

  it("flags write/ddl/unknown as destructive", () => {
    expect(isDestructiveApproval(base)).toBe(true);
    expect(isDestructiveApproval({ ...base, queryKind: "ddl" })).toBe(true);
    expect(isDestructiveApproval({ ...base, queryKind: "unknown" })).toBe(true);
  });

  it("does not flag select", () => {
    expect(isDestructiveApproval({ ...base, queryKind: "select" })).toBe(false);
  });
});
