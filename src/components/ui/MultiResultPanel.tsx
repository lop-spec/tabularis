import { useTranslation } from "react-i18next";
import {
  Play,
  Check,
  XCircle,
  Loader2,
  ChevronLeft,
  ChevronRight,
  ChevronsLeft,
  ChevronsRight,
} from "lucide-react";
import clsx from "clsx";
import { DataGrid } from "./DataGrid";
import { ErrorDisplay } from "./ErrorDisplay";
import { formatDuration } from "../../utils/formatTime";
import {
  findActiveEntry,
  countSucceeded,
  countFailed,
  totalExecutionTime,
} from "../../utils/multiResult";
import type { QueryResultEntry } from "../../types/editor";

interface MultiResultPanelProps {
  results: QueryResultEntry[];
  activeResultId: string | undefined;
  tabId: string;
  isAllDone: boolean;
  connectionId: string | null;
  copyFormat: "csv" | "json";
  csvDelimiter: string;
  onSelectResult: (entryId: string) => void;
  onRerunEntry: (entryId: string) => void;
  onPageChange: (entryId: string, page: number) => void;
}

export function MultiResultPanel({
  results,
  activeResultId,
  tabId,
  isAllDone,
  connectionId,
  copyFormat,
  csvDelimiter,
  onSelectResult,
  onRerunEntry,
  onPageChange,
}: MultiResultPanelProps) {
  const { t } = useTranslation();
  const activeEntry = findActiveEntry(results, activeResultId);
  const succeeded = countSucceeded(results);
  const failed = countFailed(results);
  const totalTime = totalExecutionTime(results);

  if (!activeEntry) return null;

  return (
    <div className="flex-1 min-h-0 flex flex-col">
      {/* Multi-result tab bar */}
      <div className="shrink-0 border-b border-default bg-elevated">
        <div className="flex items-center overflow-x-auto scrollbar-thin">
          {results.map((entry) => (
            <button
              key={entry.id}
              onClick={() => onSelectResult(entry.id)}
              className={clsx(
                "flex items-center gap-1.5 px-3 py-1.5 text-xs border-r border-default shrink-0 transition-colors",
                entry.id === activeEntry.id
                  ? "bg-surface-secondary text-white border-b-2 border-b-blue-500"
                  : "text-secondary hover:text-white hover:bg-surface-secondary/50",
              )}
            >
              {entry.isLoading ? (
                <Loader2
                  size={12}
                  className="animate-spin text-blue-400"
                />
              ) : entry.error ? (
                <XCircle size={12} className="text-red-400" />
              ) : (
                <Check size={12} className="text-green-400" />
              )}
              <span>
                {t("editor.multiResult.query", {
                  index: entry.queryIndex + 1,
                })}
              </span>
              <span
                className="max-w-[120px] truncate text-muted text-[10px]"
                title={entry.query}
              >
                {entry.query.slice(0, 40)}
              </span>
              {!entry.isLoading && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onRerunEntry(entry.id);
                  }}
                  className="ml-1 p-0.5 rounded hover:bg-surface-tertiary text-muted hover:text-blue-400 transition-colors"
                  title={t("editor.multiResult.rerun")}
                >
                  <Play size={10} fill="currentColor" />
                </button>
              )}
            </button>
          ))}
        </div>
        {/* Summary line */}
        {isAllDone && (
          <div className="px-3 py-1 text-[10px] text-muted border-t border-default bg-elevated/50">
            {t("editor.multiResult.summary", {
              total: results.length,
              succeeded,
              failed,
            })}
            {totalTime > 0 && (
              <span className="ml-2 font-mono">
                ({formatDuration(totalTime)})
              </span>
            )}
          </div>
        )}
      </div>

      {/* Active entry content */}
      <div className="flex-1 min-h-0 flex flex-col">
        {activeEntry.isLoading ? (
          <div className="flex flex-col items-center justify-center h-full text-muted">
            <div className="w-12 h-12 border-4 border-surface-secondary border-t-blue-500 rounded-full animate-spin mb-4"></div>
            <p className="text-sm">{t("editor.executingQuery")}</p>
          </div>
        ) : activeEntry.error ? (
          <ErrorDisplay error={activeEntry.error} t={t} />
        ) : activeEntry.result ? (
          <div className="flex-1 min-h-0 flex flex-col">
            <div className="p-2 bg-elevated text-xs text-secondary border-b border-default flex justify-between items-center shrink-0">
              <div className="flex items-center gap-4">
                <span>
                  {t("editor.rowsRetrieved", {
                    count: activeEntry.result.rows.length,
                  })}{" "}
                  {activeEntry.executionTime !== null && (
                    <span className="text-muted ml-2 font-mono">
                      ({formatDuration(activeEntry.executionTime)})
                    </span>
                  )}
                </span>
                {activeEntry.result.pagination?.has_more && (
                  <span className="px-2 py-0.5 bg-yellow-900/30 text-yellow-400 rounded text-[10px] font-semibold uppercase tracking-wide border border-yellow-500/30">
                    {t("editor.autoPaginated")}
                  </span>
                )}
              </div>
              {/* Pagination Controls */}
              {activeEntry.result.pagination && (
                <div className="flex items-center gap-1 bg-surface-secondary rounded border border-strong">
                  <button
                    disabled={
                      activeEntry.result.pagination.page === 1 ||
                      activeEntry.isLoading
                    }
                    onClick={() => onPageChange(activeEntry.id, 1)}
                    className="p-1 hover:bg-surface-tertiary text-secondary hover:text-white disabled:opacity-30 disabled:cursor-not-allowed"
                    title="First Page"
                  >
                    <ChevronsLeft size={14} />
                  </button>
                  <button
                    disabled={
                      activeEntry.result.pagination.page === 1 ||
                      activeEntry.isLoading
                    }
                    onClick={() =>
                      onPageChange(
                        activeEntry.id,
                        activeEntry.result!.pagination!.page - 1,
                      )
                    }
                    className="p-1 hover:bg-surface-tertiary text-secondary hover:text-white disabled:opacity-30 disabled:cursor-not-allowed border-l border-strong"
                    title="Previous Page"
                  >
                    <ChevronLeft size={14} />
                  </button>
                  <div className="px-3 text-secondary text-xs font-medium min-w-[80px] text-center py-1">
                    {activeEntry.result.pagination.total_rows !== null
                      ? t("editor.pageOf", {
                          current: activeEntry.result.pagination.page,
                          total: Math.ceil(
                            activeEntry.result.pagination.total_rows /
                              activeEntry.result.pagination.page_size,
                          ),
                        })
                      : t("editor.page", {
                          current: activeEntry.result.pagination.page,
                        })}
                  </div>
                  <button
                    disabled={
                      !activeEntry.result.pagination.has_more ||
                      activeEntry.isLoading
                    }
                    onClick={() =>
                      onPageChange(
                        activeEntry.id,
                        activeEntry.result!.pagination!.page + 1,
                      )
                    }
                    className="p-1 hover:bg-surface-tertiary text-secondary hover:text-white disabled:opacity-30 disabled:cursor-not-allowed border-l border-strong"
                    title="Next Page"
                  >
                    <ChevronRight size={14} />
                  </button>
                  <button
                    disabled={
                      activeEntry.result.pagination.total_rows === null ||
                      activeEntry.isLoading
                    }
                    onClick={() =>
                      onPageChange(
                        activeEntry.id,
                        Math.ceil(
                          activeEntry.result!.pagination!.total_rows! /
                            activeEntry.result!.pagination!.page_size,
                        ),
                      )
                    }
                    className="p-1 hover:bg-surface-tertiary text-secondary hover:text-white disabled:opacity-30 disabled:cursor-not-allowed border-l border-strong"
                    title="Last Page"
                  >
                    <ChevronsRight size={14} />
                  </button>
                </div>
              )}
            </div>
            <div className="flex-1 min-h-0 overflow-hidden">
              <DataGrid
                key={`${activeEntry.id}-${activeEntry.result.rows.length}`}
                columns={activeEntry.result.columns}
                data={activeEntry.result.rows}
                tableName={null}
                pkColumn={null}
                connectionId={connectionId}
                selectedRows={new Set()}
                onSelectionChange={() => {}}
                copyFormat={copyFormat}
                csvDelimiter={csvDelimiter}
                readonly={true}
              />
            </div>
          </div>
        ) : (
          <div className="flex items-center justify-center h-full text-surface-tertiary text-sm">
            {t("editor.executePrompt")}
          </div>
        )}
      </div>
    </div>
  );
}
