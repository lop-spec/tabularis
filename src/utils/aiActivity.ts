// Pure helpers used by the AI activity panel and approval modal.
// Kept dependency-free so they can be unit tested in isolation.

import type {
  AiActivityEvent,
  AiActivityStatus,
  AiNotebookExport,
  AiQueryKind,
  PendingApproval,
} from "../types/ai";

export interface BadgeStyle {
  bg: string;
  text: string;
  border: string;
}

const STATUS_STYLES: Record<AiActivityStatus, BadgeStyle> = {
  success: {
    bg: "bg-green-900/20",
    text: "text-green-400",
    border: "border-green-900/40",
  },
  blocked_readonly: {
    bg: "bg-yellow-900/20",
    text: "text-yellow-400",
    border: "border-yellow-900/40",
  },
  blocked_pending_approval: {
    bg: "bg-purple-900/20",
    text: "text-purple-400",
    border: "border-purple-900/40",
  },
  denied: {
    bg: "bg-red-900/20",
    text: "text-red-400",
    border: "border-red-900/40",
  },
  error: {
    bg: "bg-red-900/20",
    text: "text-red-400",
    border: "border-red-900/40",
  },
  timeout: {
    bg: "bg-orange-900/20",
    text: "text-orange-400",
    border: "border-orange-900/40",
  },
};

const QUERY_KIND_STYLES: Record<AiQueryKind, BadgeStyle> = {
  select: {
    bg: "bg-blue-900/20",
    text: "text-blue-400",
    border: "border-blue-900/40",
  },
  write: {
    bg: "bg-yellow-900/20",
    text: "text-yellow-400",
    border: "border-yellow-900/40",
  },
  ddl: {
    bg: "bg-orange-900/20",
    text: "text-orange-400",
    border: "border-orange-900/40",
  },
  unknown: {
    bg: "bg-surface-secondary",
    text: "text-muted",
    border: "border-default",
  },
};

const FALLBACK_STYLE: BadgeStyle = {
  bg: "bg-surface-secondary",
  text: "text-muted",
  border: "border-default",
};

export function getStatusBadgeStyle(status: string): BadgeStyle {
  return STATUS_STYLES[status as AiActivityStatus] ?? FALLBACK_STYLE;
}

export function getQueryKindBadgeStyle(kind: string | null): BadgeStyle {
  if (!kind) return FALLBACK_STYLE;
  return QUERY_KIND_STYLES[kind as AiQueryKind] ?? FALLBACK_STYLE;
}

export function truncateQuery(query: string | null, maxLen = 80): string {
  if (!query) return "";
  const oneLine = query.replace(/\s+/g, " ").trim();
  if (oneLine.length <= maxLen) return oneLine;
  return `${oneLine.slice(0, maxLen)}…`;
}

export function formatDurationMs(ms: number): string {
  if (ms < 1) return "<1 ms";
  if (ms < 1_000) return `${Math.round(ms)} ms`;
  if (ms < 60_000) return `${(ms / 1_000).toFixed(2)} s`;
  const minutes = Math.floor(ms / 60_000);
  const seconds = Math.round((ms % 60_000) / 1_000);
  return `${minutes}m ${seconds}s`;
}

export function eventsToCsvLines(events: AiActivityEvent[]): string[] {
  const header = [
    "id",
    "session_id",
    "timestamp",
    "tool",
    "connection_name",
    "query_kind",
    "duration_ms",
    "status",
    "rows",
    "error",
  ].join(",");
  const escape = (v: string | number | null | undefined): string => {
    if (v === null || v === undefined) return "";
    const s = String(v);
    if (/[",\n]/.test(s)) return `"${s.replace(/"/g, '""')}"`;
    return s;
  };
  const rows = events.map((e) =>
    [
      escape(e.id),
      escape(e.sessionId),
      escape(e.timestamp),
      escape(e.tool),
      escape(e.connectionName),
      escape(e.queryKind),
      escape(e.durationMs),
      escape(e.status),
      escape(e.rows),
      escape(e.error),
    ].join(","),
  );
  return [header, ...rows];
}

/// Convert the backend export shape to a `NotebookFile`-compatible payload
/// that can be passed to the existing `save_notebook` / notebook editor.
export function notebookFileFromExport(
  exportData: AiNotebookExport,
): NotebookFilePayload {
  return {
    version: exportData.version,
    title: exportData.title,
    createdAt: exportData.createdAt,
    cells: exportData.cells.map((c) => ({
      type: c.type,
      content: c.content,
      ...(c.name !== undefined && { name: c.name }),
      ...(c.schema !== undefined && { schema: c.schema }),
    })),
  };
}

/// Minimal NotebookFile shape — kept here to avoid importing the larger
/// NotebookFile type into a leaf utility module.
export interface NotebookFilePayload {
  version: number;
  title: string;
  createdAt: string;
  cells: Array<{
    type: "sql" | "markdown";
    content: string;
    name?: string;
    schema?: string;
  }>;
}

/// Build a default i18n filename for a notebook export, e.g.
/// `ai-session-2026-04-24-claude-desktop.tabularis-notebook`.
export function defaultExportFilename(
  sessionId: string,
  exportData: AiNotebookExport,
): string {
  const dateOnly = exportData.createdAt.slice(0, 10);
  const slug = sessionId.slice(0, 8);
  return `ai-session-${dateOnly}-${slug}.tabularis-notebook`;
}

/// Group events by session id, preserving the original order.
export function groupBySession(
  events: AiActivityEvent[],
): Map<string, AiActivityEvent[]> {
  const map = new Map<string, AiActivityEvent[]>();
  for (const ev of events) {
    const list = map.get(ev.sessionId) ?? [];
    list.push(ev);
    map.set(ev.sessionId, list);
  }
  return map;
}

/// Build a deep-link URL pointing at the Visual Explain page with a query
/// pre-populated. The query is base64url-encoded so it survives URL escaping.
export function buildVisualExplainDeepLink(
  connectionId: string,
  query: string,
): string {
  return `/visual-explain?connection=${encodeURIComponent(
    connectionId,
  )}&query=${base64UrlEncode(query)}`;
}

export function parseVisualExplainDeepLink(search: string): {
  connectionId: string | null;
  query: string | null;
} {
  const params = new URLSearchParams(search.startsWith("?") ? search : `?${search}`);
  const connectionId = params.get("connection");
  const encoded = params.get("query");
  const query = encoded ? base64UrlDecode(encoded) : null;
  return { connectionId, query };
}

function base64UrlEncode(s: string): string {
  const bytes =
    typeof TextEncoder !== "undefined"
      ? new TextEncoder().encode(s)
      : Uint8Array.from(s, (c) => c.charCodeAt(0));
  let binary = "";
  bytes.forEach((b) => {
    binary += String.fromCharCode(b);
  });
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function base64UrlDecode(s: string): string {
  let padded = s.replace(/-/g, "+").replace(/_/g, "/");
  while (padded.length % 4 !== 0) padded += "=";
  const binary = atob(padded);
  const bytes = Uint8Array.from(binary, (c) => c.charCodeAt(0));
  return typeof TextDecoder !== "undefined"
    ? new TextDecoder().decode(bytes)
    : binary;
}

/// Returns true when the pending approval would be considered destructive
/// (write or DDL). Use to drive UI emphasis (red border, etc).
export function isDestructiveApproval(p: PendingApproval): boolean {
  return p.queryKind === "write" || p.queryKind === "ddl" || p.queryKind === "unknown";
}
