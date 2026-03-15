import { useState, useMemo } from "react";
import { ChevronDown, ChevronRight, Copy, Check } from "lucide-react";
import type { SlotComponentProps } from "../../../../types/pluginSlots";

/** JSON type categories for syntax coloring. */
type JsonTokenType = "string" | "number" | "boolean" | "null" | "key";

const TOKEN_COLORS: Record<JsonTokenType, string> = {
  string: "text-green-400",
  number: "text-blue-400",
  boolean: "text-yellow-400",
  null: "text-red-400/70",
  key: "text-purple-300",
};

/** Check if a column data type looks like JSON/JSONB. */
function isJsonColumn(columnName: string, value: unknown): boolean {
  const nameLower = columnName.toLowerCase();
  if (nameLower.includes("json")) return true;

  if (typeof value === "string") {
    const trimmed = value.trim();
    return (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
           (trimmed.startsWith("[") && trimmed.endsWith("]"));
  }

  if (typeof value === "object" && value !== null) return true;

  return false;
}

/** Try to parse a value as JSON. Returns [parsed, isValid]. */
function tryParseJson(value: unknown): [unknown, boolean] {
  if (typeof value === "object" && value !== null) return [value, true];
  if (typeof value !== "string") return [null, false];

  try {
    const parsed = JSON.parse(value);
    if (typeof parsed === "object" && parsed !== null) return [parsed, true];
    return [null, false];
  } catch {
    return [null, false];
  }
}

/** Render a JSON value with syntax highlighting. */
function JsonValue({ value, depth, defaultExpanded }: { value: unknown; depth: number; defaultExpanded: boolean }) {
  if (value === null) return <span className={TOKEN_COLORS.null}>null</span>;
  if (typeof value === "boolean") return <span className={TOKEN_COLORS.boolean}>{String(value)}</span>;
  if (typeof value === "number") return <span className={TOKEN_COLORS.number}>{value}</span>;
  if (typeof value === "string") {
    // Truncate long strings
    const display = value.length > 120 ? value.slice(0, 120) + "..." : value;
    return <span className={TOKEN_COLORS.string}>&quot;{display}&quot;</span>;
  }
  if (Array.isArray(value)) {
    return <JsonArray items={value} depth={depth} defaultExpanded={defaultExpanded} />;
  }
  if (typeof value === "object") {
    return <JsonObject obj={value as Record<string, unknown>} depth={depth} defaultExpanded={defaultExpanded} />;
  }
  return <span>{String(value)}</span>;
}

/** Collapsible JSON object renderer. */
function JsonObject({ obj, depth, defaultExpanded }: { obj: Record<string, unknown>; depth: number; defaultExpanded: boolean }) {
  const [expanded, setExpanded] = useState(defaultExpanded && depth < 2);
  const entries = Object.entries(obj);

  if (entries.length === 0) return <span className="text-muted">{"{}"}</span>;

  if (!expanded) {
    return (
      <span
        className="cursor-pointer hover:opacity-80 inline-flex items-center gap-0.5"
        onClick={() => setExpanded(true)}
      >
        <ChevronRight size={10} className="text-muted shrink-0" />
        <span className="text-muted">{"{"}</span>
        <span className="text-muted/60 text-[10px]">{entries.length} keys</span>
        <span className="text-muted">{"}"}</span>
      </span>
    );
  }

  return (
    <span>
      <span
        className="cursor-pointer hover:opacity-80 inline-flex items-center gap-0.5"
        onClick={() => setExpanded(false)}
      >
        <ChevronDown size={10} className="text-muted shrink-0" />
        <span className="text-muted">{"{"}</span>
      </span>
      <div className="ml-4 border-l border-default/30 pl-2">
        {entries.map(([key, val], i) => (
          <div key={key} className="leading-relaxed">
            <span className={TOKEN_COLORS.key}>&quot;{key}&quot;</span>
            <span className="text-muted">: </span>
            <JsonValue value={val} depth={depth + 1} defaultExpanded={defaultExpanded} />
            {i < entries.length - 1 && <span className="text-muted">,</span>}
          </div>
        ))}
      </div>
      <span className="text-muted">{"}"}</span>
    </span>
  );
}

/** Collapsible JSON array renderer. */
function JsonArray({ items, depth, defaultExpanded }: { items: unknown[]; depth: number; defaultExpanded: boolean }) {
  const [expanded, setExpanded] = useState(defaultExpanded && depth < 2);

  if (items.length === 0) return <span className="text-muted">[]</span>;

  if (!expanded) {
    return (
      <span
        className="cursor-pointer hover:opacity-80 inline-flex items-center gap-0.5"
        onClick={() => setExpanded(true)}
      >
        <ChevronRight size={10} className="text-muted shrink-0" />
        <span className="text-muted">[</span>
        <span className="text-muted/60 text-[10px]">{items.length} items</span>
        <span className="text-muted">]</span>
      </span>
    );
  }

  return (
    <span>
      <span
        className="cursor-pointer hover:opacity-80 inline-flex items-center gap-0.5"
        onClick={() => setExpanded(false)}
      >
        <ChevronDown size={10} className="text-muted shrink-0" />
        <span className="text-muted">[</span>
      </span>
      <div className="ml-4 border-l border-default/30 pl-2">
        {items.map((item, i) => (
          <div key={i} className="leading-relaxed">
            <JsonValue value={item} depth={depth + 1} defaultExpanded={defaultExpanded} />
            {i < items.length - 1 && <span className="text-muted">,</span>}
          </div>
        ))}
      </div>
      <span className="text-muted">]</span>
    </span>
  );
}

/** Main JSON Viewer slot component. */
export default function JsonPreview({ context }: SlotComponentProps) {
  const { columnName, rowData } = context;
  const [copied, setCopied] = useState(false);

  const rawValue = columnName && rowData ? rowData[columnName] : undefined;

  const [parsed, isValid] = useMemo(
    () => (rawValue !== undefined && rawValue !== null && rawValue !== "")
      ? tryParseJson(rawValue)
      : [null, false],
    [rawValue],
  );

  // Only render for JSON-like columns with valid data
  if (!columnName || !isValid || !isJsonColumn(columnName, rawValue)) return null;

  const handleCopy = async () => {
    const text = typeof rawValue === "string" ? rawValue : JSON.stringify(parsed, null, 2);
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <div className="mt-1.5 rounded-lg border border-default/50 bg-base/50 overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-2.5 py-1 border-b border-default/30 bg-surface-secondary/30">
        <span className="text-[10px] font-medium text-muted uppercase tracking-wider">
          JSON Preview
        </span>
        <button
          onClick={handleCopy}
          className="flex items-center gap-1 text-[10px] text-muted hover:text-secondary transition-colors"
          title="Copy formatted JSON"
        >
          {copied ? <Check size={10} className="text-green-400" /> : <Copy size={10} />}
          {copied ? "Copied" : "Copy"}
        </button>
      </div>

      {/* JSON tree */}
      <div className="px-2.5 py-2 text-[11px] font-mono leading-relaxed max-h-64 overflow-auto">
        <JsonValue value={parsed} depth={0} defaultExpanded={true} />
      </div>
    </div>
  );
}
