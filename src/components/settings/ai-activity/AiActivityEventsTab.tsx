import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Copy, Eye, GitGraph, Trash2, Download, RefreshCw } from "lucide-react";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import {
  clearAiActivity,
  exportAiActivityCsv,
  exportAiActivityJson,
  useAiActivityEvents,
} from "../../../hooks/useAiActivity";
import { formatDurationMs, truncateQuery } from "../../../utils/aiActivity";
import type { AiActivityEvent, AiEventFilter } from "../../../types/ai";
import { useAlert } from "../../../hooks/useAlert";
import { StatusBadge } from "./StatusBadge";
import { QueryKindBadge } from "./QueryKindBadge";
import { EventDetailModal } from "./EventDetailModal";
import { VisualExplainModal } from "../../modals/VisualExplainModal";

interface ExplainTarget {
  query: string;
  connectionId: string;
  connectionName: string | null;
}

export function AiActivityEventsTab() {
  const { t } = useTranslation();
  const { showAlert } = useAlert();
  const [filter, setFilter] = useState<AiEventFilter>({});
  const [detail, setDetail] = useState<AiActivityEvent | null>(null);
  const [explainTarget, setExplainTarget] = useState<ExplainTarget | null>(
    null,
  );
  const { events, loading, refetch } = useAiActivityEvents(filter);

  const stats = useMemo(() => {
    const errors = events.filter(
      (e) => e.status === "error" || e.status === "denied" || e.status === "timeout",
    ).length;
    const blocked = events.filter((e) =>
      e.status.startsWith("blocked"),
    ).length;
    return { total: events.length, errors, blocked };
  }, [events]);

  const handleClear = async () => {
    if (!confirm(t("aiActivity.clearConfirm"))) return;
    try {
      await clearAiActivity();
      await refetch();
    } catch (err) {
      showAlert(String(err), { kind: "error", title: t("common.error") });
    }
  };

  const handleExport = async (format: "json" | "csv") => {
    try {
      const content =
        format === "json" ? await exportAiActivityJson() : await exportAiActivityCsv();
      const target = await saveDialog({
        defaultPath:
          format === "json" ? "ai-activity.jsonl" : "ai-activity.csv",
        filters: [
          {
            name: format === "json" ? "JSON Lines" : "CSV",
            extensions: [format === "json" ? "jsonl" : "csv"],
          },
        ],
      });
      if (typeof target === "string" && target.length > 0) {
        await writeTextFile(target, content);
        showAlert(t("aiActivity.exportSuccess", { path: target }), {
          kind: "info",
        });
      }
    } catch (err) {
      showAlert(String(err), { kind: "error", title: t("common.error") });
    }
  };

  const handleOpenInVisualExplain = (ev: AiActivityEvent) => {
    if (!ev.query || !ev.connectionId) return;
    setExplainTarget({
      query: ev.query,
      connectionId: ev.connectionId,
      connectionName: ev.connectionName,
    });
  };

  return (
    <div className="space-y-4">
      <FiltersBar
        filter={filter}
        onFilterChange={setFilter}
        onRefresh={refetch}
        onClear={handleClear}
        onExportJson={() => handleExport("json")}
        onExportCsv={() => handleExport("csv")}
      />

      <div className="flex items-center gap-3 text-xs text-muted">
        <span>
          {t("aiActivity.eventsCount", { count: stats.total })}
        </span>
        {stats.blocked > 0 && (
          <span className="text-yellow-400">
            · {t("aiActivity.blockedCount", { count: stats.blocked })}
          </span>
        )}
        {stats.errors > 0 && (
          <span className="text-red-400">
            · {t("aiActivity.errorsCount", { count: stats.errors })}
          </span>
        )}
      </div>

      <EventsTable
        events={events}
        loading={loading}
        onView={setDetail}
        onCopyQuery={(q) => {
          navigator.clipboard.writeText(q);
          showAlert(t("aiActivity.copied"), { kind: "info" });
        }}
        onOpenInVisualExplain={handleOpenInVisualExplain}
      />

      {detail && (
        <EventDetailModal event={detail} onClose={() => setDetail(null)} />
      )}

      <VisualExplainModal
        isOpen={explainTarget !== null}
        onClose={() => setExplainTarget(null)}
        query={explainTarget?.query ?? ""}
        connectionId={explainTarget?.connectionId ?? ""}
        connectionLabel={explainTarget?.connectionName ?? undefined}
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Sub-components — kept inside the file because they only make sense inside
// the events tab and never get reused.
// ---------------------------------------------------------------------------

interface FiltersBarProps {
  filter: AiEventFilter;
  onFilterChange: (f: AiEventFilter) => void;
  onRefresh: () => void;
  onClear: () => void;
  onExportJson: () => void;
  onExportCsv: () => void;
}

function FiltersBar({
  filter,
  onFilterChange,
  onRefresh,
  onClear,
  onExportJson,
  onExportCsv,
}: FiltersBarProps) {
  const { t } = useTranslation();
  const update = (patch: Partial<AiEventFilter>) =>
    onFilterChange({ ...filter, ...patch });

  return (
    <div className="bg-surface-secondary/40 rounded-lg p-3 flex flex-wrap items-center gap-2 border border-default">
      <input
        type="text"
        placeholder={t("aiActivity.searchQuery")}
        value={filter.queryContains ?? ""}
        onChange={(e) =>
          update({ queryContains: e.target.value || undefined })
        }
        className="flex-1 min-w-[180px] bg-base border border-strong rounded px-3 py-1.5 text-xs text-primary focus:outline-none focus:border-blue-500"
      />
      <select
        value={filter.tool ?? ""}
        onChange={(e) => update({ tool: e.target.value || undefined })}
        className="bg-base border border-strong rounded px-2 py-1.5 text-xs text-primary focus:outline-none focus:border-blue-500"
      >
        <option value="">{t("aiActivity.allTools")}</option>
        <option value="list_connections">list_connections</option>
        <option value="list_tables">list_tables</option>
        <option value="describe_table">describe_table</option>
        <option value="run_query">run_query</option>
      </select>
      <select
        value={filter.status ?? ""}
        onChange={(e) =>
          update({
            status: (e.target.value || undefined) as AiEventFilter["status"],
          })
        }
        className="bg-base border border-strong rounded px-2 py-1.5 text-xs text-primary focus:outline-none focus:border-blue-500"
      >
        <option value="">{t("aiActivity.allStatuses")}</option>
        <option value="success">{t("aiActivity.status.success")}</option>
        <option value="error">{t("aiActivity.status.error")}</option>
        <option value="denied">{t("aiActivity.status.denied")}</option>
        <option value="timeout">{t("aiActivity.status.timeout")}</option>
        <option value="blocked_readonly">
          {t("aiActivity.status.blocked_readonly")}
        </option>
      </select>
      <button
        onClick={onRefresh}
        className="p-1.5 text-muted hover:text-primary hover:bg-surface-tertiary rounded transition-colors"
        title={t("common.refresh", { defaultValue: "Refresh" })}
      >
        <RefreshCw size={14} />
      </button>
      <button
        onClick={onExportCsv}
        className="flex items-center gap-1 px-2.5 py-1.5 text-xs text-muted hover:text-primary hover:bg-surface-tertiary rounded transition-colors"
      >
        <Download size={12} /> CSV
      </button>
      <button
        onClick={onExportJson}
        className="flex items-center gap-1 px-2.5 py-1.5 text-xs text-muted hover:text-primary hover:bg-surface-tertiary rounded transition-colors"
      >
        <Download size={12} /> JSON
      </button>
      <button
        onClick={onClear}
        className="flex items-center gap-1 px-2.5 py-1.5 text-xs text-red-400 hover:text-red-300 hover:bg-red-900/20 rounded transition-colors"
      >
        <Trash2 size={12} /> {t("aiActivity.clearAll")}
      </button>
    </div>
  );
}

interface EventsTableProps {
  events: AiActivityEvent[];
  loading: boolean;
  onView: (event: AiActivityEvent) => void;
  onCopyQuery: (query: string) => void;
  onOpenInVisualExplain: (event: AiActivityEvent) => void;
}

function EventsTable({
  events,
  loading,
  onView,
  onCopyQuery,
  onOpenInVisualExplain,
}: EventsTableProps) {
  const { t } = useTranslation();
  if (loading && events.length === 0) {
    return (
      <div className="text-center py-12 text-muted text-sm">
        {t("common.loading")}
      </div>
    );
  }
  if (events.length === 0) {
    return (
      <div className="text-center py-12 text-muted text-sm">
        {t("aiActivity.empty")}
      </div>
    );
  }
  return (
    <div className="border border-default rounded-lg overflow-hidden">
      <div className="max-h-[500px] overflow-auto">
        <table className="w-full text-xs">
          <thead className="bg-surface-secondary sticky top-0">
            <tr className="text-muted text-left">
              <th className="px-3 py-2 font-medium">
                {t("aiActivity.col.timestamp")}
              </th>
              <th className="px-3 py-2 font-medium">{t("aiActivity.col.tool")}</th>
              <th className="px-3 py-2 font-medium">
                {t("aiActivity.col.connection")}
              </th>
              <th className="px-3 py-2 font-medium">{t("aiActivity.col.query")}</th>
              <th className="px-3 py-2 font-medium">{t("aiActivity.col.kind")}</th>
              <th className="px-3 py-2 font-medium text-right">
                {t("aiActivity.col.duration")}
              </th>
              <th className="px-3 py-2 font-medium">{t("aiActivity.col.status")}</th>
              <th className="px-3 py-2 font-medium text-right">
                {t("aiActivity.col.actions")}
              </th>
            </tr>
          </thead>
          <tbody>
            {events.map((ev) => (
              <tr
                key={ev.id}
                className="border-t border-default hover:bg-surface-tertiary/30"
              >
                <td className="px-3 py-2 text-muted font-mono whitespace-nowrap">
                  {ev.timestamp.replace("T", " ").slice(0, 19)}
                </td>
                <td className="px-3 py-2 text-primary font-mono">{ev.tool}</td>
                <td className="px-3 py-2 text-muted">
                  {ev.connectionName ?? "—"}
                </td>
                <td className="px-3 py-2 text-secondary font-mono max-w-[260px] truncate">
                  {truncateQuery(ev.query, 80) || "—"}
                </td>
                <td className="px-3 py-2">
                  <QueryKindBadge kind={ev.queryKind} />
                </td>
                <td className="px-3 py-2 text-muted text-right whitespace-nowrap">
                  {formatDurationMs(ev.durationMs)}
                </td>
                <td className="px-3 py-2">
                  <StatusBadge status={ev.status} />
                </td>
                <td className="px-3 py-2">
                  <div className="flex items-center justify-end gap-1">
                    <button
                      onClick={() => onView(ev)}
                      className="p-1 text-muted hover:text-primary hover:bg-surface-tertiary rounded"
                      title={t("aiActivity.viewDetails")}
                    >
                      <Eye size={12} />
                    </button>
                    {ev.query && (
                      <button
                        onClick={() => onCopyQuery(ev.query!)}
                        className="p-1 text-muted hover:text-primary hover:bg-surface-tertiary rounded"
                        title={t("aiActivity.copyQuery")}
                      >
                        <Copy size={12} />
                      </button>
                    )}
                    {ev.tool === "run_query" && ev.query && ev.connectionId && (
                      <button
                        onClick={() => onOpenInVisualExplain(ev)}
                        className="p-1 text-muted hover:text-green-400 hover:bg-green-900/20 rounded"
                        title={t("aiActivity.openVisualExplain")}
                      >
                        <GitGraph size={12} />
                      </button>
                    )}
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
