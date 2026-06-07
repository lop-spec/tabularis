import clsx from "clsx";
import { Database, Download, ShieldCheck } from "lucide-react";
import type { CSSProperties } from "react";

import type { PluginManifest } from "../../../types/plugins";
import type { CatalogueDriver, EngineGroup } from "../../../utils/connectionCatalogue";
import { getDriverIcon } from "../../../utils/driverUI";

interface EngineCardProps {
  group: EngineGroup;
  onSelect: (group: EngineGroup) => void;
}

/** Pleasant accent per data-model family, used when a driver declares no color. */
const PARADIGM_ACCENT: Record<string, string> = {
  sql: "#3b82f6",
  nosql: "#10b981",
  document: "#10b981",
  "key-value": "#14b8a6",
  vector: "#a855f7",
  graph: "#f59e0b",
  timeseries: "#ec4899",
  relational: "#3b82f6",
  other: "#64748b",
};

function accentFor(group: EngineGroup, rep: CatalogueDriver): string {
  return rep.color || PARADIGM_ACCENT[group.primaryParadigm] || "#64748b";
}

function renderIcon(rep: CatalogueDriver) {
  const icon = rep.icon ?? "";
  if (/^https?:\/\//.test(icon) || icon.startsWith("data:")) {
    return <img src={icon} alt="" className="h-6 w-6 rounded object-contain" />;
  }
  if (rep.isBuiltin) {
    return getDriverIcon({ icon, color: rep.color ?? undefined } as PluginManifest, 22);
  }
  return <Database size={20} />;
}

function formatCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
  return String(n);
}

export function EngineCard({ group, onSelect }: EngineCardProps) {
  const rep = group.drivers.find((d) => d.isBuiltin) ?? group.drivers[0];
  const accent = accentFor(group, rep);
  const driverCount = group.drivers.length;

  return (
    <button
      type="button"
      aria-label={`Connect to ${group.engine}`}
      onClick={() => onSelect(group)}
      style={{ "--accent": accent } as CSSProperties}
      className={clsx(
        "group relative flex cursor-pointer items-center gap-3 overflow-hidden rounded-xl border p-3 text-left",
        "border-default bg-surface-secondary transition-all duration-150",
        "hover:-translate-y-px hover:border-[var(--accent)] hover:bg-surface hover:shadow-md hover:shadow-black/5",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)]/50",
      )}
    >
      {/* accent rail */}
      <span
        aria-hidden
        className="absolute inset-y-0 left-0 w-0.5 opacity-0 transition-opacity group-hover:opacity-100"
        style={{ backgroundColor: accent }}
      />

      {/* icon tile */}
      <span
        className="flex h-11 w-11 shrink-0 items-center justify-center rounded-lg"
        style={{ backgroundColor: `${accent}1f`, color: accent }}
      >
        {renderIcon(rep)}
      </span>

      {/* body */}
      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="flex items-center gap-1.5">
          <span className="truncate font-semibold text-primary capitalize">{group.displayName}</span>
          {group.verified && (
            <span className="inline-flex shrink-0 items-center text-blue-400" title="Verified">
              <ShieldCheck size={13} aria-hidden />
              <span className="sr-only">Verified</span>
            </span>
          )}
        </span>
        <span className="flex items-center gap-1 text-[11px] text-muted">
          <span className="capitalize">{group.primaryParadigm}</span>
          {group.secondaryParadigms.length > 0 && (
            <span className="text-muted/70">· +{group.secondaryParadigms.length}</span>
          )}
          {driverCount > 1 && <span className="text-muted/70">· {driverCount} drivers</span>}
        </span>
      </span>

      {/* trailing status */}
      <span className="flex shrink-0 flex-col items-end gap-1">
        {group.installed ? (
          <span className="flex items-center gap-1 rounded-full bg-emerald-500/15 px-2 py-0.5 text-[10px] font-semibold text-emerald-400">
            <span className="h-1.5 w-1.5 rounded-full bg-emerald-400" />
            Installed
          </span>
        ) : (
          <span className="rounded-full border border-default px-2 py-0.5 text-[10px] font-medium text-muted opacity-0 transition-opacity group-hover:opacity-100">
            Install
          </span>
        )}
        {group.downloads != null && (
          <span className="flex items-center gap-0.5 text-[10px] text-muted">
            <Download size={10} />
            {formatCount(group.downloads)}
          </span>
        )}
      </span>
    </button>
  );
}
