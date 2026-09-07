import { useState, useEffect } from "react";
import { loader } from "@monaco-editor/react";
import DOMPurify from "dompurify";
import type * as Monaco from "monaco-editor";

let monacoInstance: typeof Monaco | null = null;
let monacoReady: Promise<typeof Monaco> | null = null;
const inFlight = new Map<string, Promise<string>>();

function loadMonaco(): Promise<typeof Monaco> {
  if (!monacoReady) {
    monacoReady = loader.init().then((monaco: typeof Monaco) => {
      monacoInstance = monaco;
      return monaco;
    }).catch((error: unknown) => {
      monacoReady = null;
      throw error;
    });
  }
  return monacoReady;
}

// Share concurrent previews, but never retain theme-dependent HTML after completion.
function colorize(sql: string): Promise<string> {
  const pending = inFlight.get(sql);
  if (pending) return pending;
  const result = loadMonaco()
    .then((monaco) => monaco.editor.colorize(sql, "sql", { tabSize: 2 }))
    .then((html) => DOMPurify.sanitize(html, {
      ALLOWED_TAGS: ["span", "br"],
      ALLOWED_ATTR: ["class"],
    }));
  if (inFlight.size < 64 && sql.length <= 64 * 1024) {
    inFlight.set(sql, result);
    const remove = () => { if (inFlight.get(sql) === result) inFlight.delete(sql); };
    void result.then(remove, remove);
  }
  return result;
}

/** Syntax-highlighted HTML, sanitized before reaching the DOM. */
export function useColorizedSql(sql: string): string | null {
  const [preview, setPreview] = useState<{ sql: string; html: string } | null>(null);
  useEffect(() => {
    let cancelled = false;
    void colorize(sql).then((html) => {
      if (!cancelled) setPreview({ sql, html });
    }).catch((error: unknown) => {
      console.error("[sql-highlight] Colorization failed; using plain text", error);
    });
    return () => { cancelled = true; };
  }, [sql]);
  return preview?.sql === sql ? preview.html : null;
}

/** Returns a pending preview if Monaco is already loaded, otherwise null. */
export function colorizeSqlSync(sql: string): Promise<string> | null {
  if (!monacoInstance) return null;
  return colorize(sql);
}

export function formatSqlPreview(sql: string, maxLines = 3): string {
  const limit = Math.max(0, Math.floor(maxLines));
  const lines: string[] = [];
  let start = 0;
  while (start <= sql.length) {
    const end = sql.indexOf("\n", start);
    const line = sql.slice(start, end < 0 ? sql.length : end).trim();
    if (line) {
      if (lines.length === limit) return lines.join("\n") + " ...";
      lines.push(line);
    }
    if (end < 0) break;
    start = end + 1;
  }
  return lines.join("\n");
}
