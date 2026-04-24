import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronRight, FileDown, RefreshCw } from "lucide-react";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import {
  exportSessionAsNotebook,
  useAiSessionEvents,
  useAiSessions,
} from "../../../hooks/useAiActivity";
import { useAlert } from "../../../hooks/useAlert";
import {
  defaultExportFilename,
  formatDurationMs,
  notebookFileFromExport,
  truncateQuery,
} from "../../../utils/aiActivity";
import type { AiSessionSummary } from "../../../types/ai";
import { StatusBadge } from "./StatusBadge";

export function AiActivitySessionsTab() {
  const { t } = useTranslation();
  const { sessions, loading, refetch } = useAiSessions();
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <span className="text-xs text-muted">
          {t("aiActivity.sessionsCount", { count: sessions.length })}
        </span>
        <button
          onClick={refetch}
          className="p-1.5 text-muted hover:text-primary hover:bg-surface-tertiary rounded transition-colors"
          title={t("common.refresh", { defaultValue: "Refresh" })}
        >
          <RefreshCw size={14} />
        </button>
      </div>

      {loading && sessions.length === 0 ? (
        <div className="text-center py-12 text-muted text-sm">
          {t("common.loading")}
        </div>
      ) : sessions.length === 0 ? (
        <div className="text-center py-12 text-muted text-sm">
          {t("aiActivity.empty")}
        </div>
      ) : (
        <div className="space-y-2">
          {sessions.map((s) => (
            <SessionCard
              key={s.sessionId}
              session={s}
              expanded={activeSessionId === s.sessionId}
              onToggle={() =>
                setActiveSessionId((cur) =>
                  cur === s.sessionId ? null : s.sessionId,
                )
              }
            />
          ))}
        </div>
      )}
    </div>
  );
}

interface SessionCardProps {
  session: AiSessionSummary;
  expanded: boolean;
  onToggle: () => void;
}

function SessionCard({ session, expanded, onToggle }: SessionCardProps) {
  const { t } = useTranslation();
  const { showAlert } = useAlert();

  const handleExport = async () => {
    try {
      const exp = await exportSessionAsNotebook(session.sessionId);
      const file = notebookFileFromExport(exp);
      const target = await saveDialog({
        defaultPath: defaultExportFilename(session.sessionId, exp),
        filters: [
          {
            name: "Tabularis Notebook",
            extensions: ["tabularis-notebook"],
          },
        ],
      });
      if (typeof target === "string" && target.length > 0) {
        await writeTextFile(target, JSON.stringify(file, null, 2));
        showAlert(t("aiActivity.exportSuccess", { path: target }), {
          kind: "info",
        });
      }
    } catch (err) {
      showAlert(String(err), { kind: "error", title: t("common.error") });
    }
  };

  return (
    <div className="border border-default rounded-lg overflow-hidden bg-surface-secondary/30">
      <button
        onClick={onToggle}
        className="w-full flex items-center justify-between p-3 hover:bg-surface-tertiary/30 transition-colors text-left"
      >
        <div className="flex items-start gap-3 min-w-0">
          <ChevronRight
            size={14}
            className={`mt-0.5 text-muted shrink-0 transition-transform ${expanded ? "rotate-90" : ""}`}
          />
          <div className="min-w-0">
            <div className="flex items-center gap-2 text-sm">
              <span className="font-mono text-primary truncate">
                {session.sessionId.slice(0, 8)}…
              </span>
              {session.clientHint && (
                <span className="px-1.5 py-0.5 text-[10px] uppercase font-medium rounded bg-blue-900/20 text-blue-400 border border-blue-900/40">
                  {session.clientHint}
                </span>
              )}
            </div>
            <div className="text-xs text-muted mt-1 flex flex-wrap gap-x-3">
              <span>
                {t("aiActivity.events")}: {session.eventCount}
              </span>
              <span>
                {t("aiActivity.runQueries")}: {session.runQueryCount}
              </span>
              {session.connectionNames.length > 0 && (
                <span className="truncate max-w-[260px]">
                  {t("aiActivity.connections")}:{" "}
                  {session.connectionNames.join(", ")}
                </span>
              )}
              <span>
                {session.startedAt.replace("T", " ").slice(0, 19)}
              </span>
            </div>
          </div>
        </div>
        <button
          onClick={(e) => {
            e.stopPropagation();
            handleExport();
          }}
          className="flex items-center gap-1.5 px-2.5 py-1 text-xs text-muted hover:text-primary hover:bg-surface-tertiary rounded transition-colors shrink-0"
        >
          <FileDown size={12} />
          {t("aiActivity.exportNotebook")}
        </button>
      </button>
      {expanded && <SessionEventList sessionId={session.sessionId} />}
    </div>
  );
}

function SessionEventList({ sessionId }: { sessionId: string }) {
  const { t } = useTranslation();
  const { events, loading } = useAiSessionEvents(sessionId);
  if (loading) {
    return (
      <div className="px-4 py-3 text-xs text-muted">{t("common.loading")}</div>
    );
  }
  if (events.length === 0) {
    return (
      <div className="px-4 py-3 text-xs text-muted">
        {t("aiActivity.empty")}
      </div>
    );
  }
  return (
    <div className="border-t border-default bg-base/40">
      {events.map((ev) => (
        <div
          key={ev.id}
          className="flex items-center gap-3 px-4 py-2 text-xs border-b border-default last:border-b-0"
        >
          <span className="text-muted font-mono whitespace-nowrap w-32">
            {ev.timestamp.replace("T", " ").slice(11, 19)}
          </span>
          <span className="text-primary font-mono w-32 shrink-0">{ev.tool}</span>
          <span className="text-secondary font-mono truncate flex-1">
            {truncateQuery(ev.query, 100) || ev.connectionName || "—"}
          </span>
          <span className="text-muted whitespace-nowrap">
            {formatDurationMs(ev.durationMs)}
          </span>
          <StatusBadge status={ev.status} />
        </div>
      ))}
    </div>
  );
}
