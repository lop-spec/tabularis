import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { History, Search, X, Loader2, AlertCircle } from "lucide-react";
import { useQueryHistory } from "../hooks/useQueryHistory";
import type { QueryHistoryEntry } from "../contexts/QueryHistoryContext";
import { formatDuration } from "../utils/formatTime";

/**
 * Full execution history, across every connection.
 *
 * Deliberately not scoped to the active connection: the sidebar section is the
 * contextual view, and this page exists to find a statement again regardless
 * of which connection is currently selected — including ones run under a
 * connection that has since been removed. Each row names its source.
 *
 * Only the newest page is loaded up front; searching streams the complete
 * on-disk logs, which is also what Recovery reads.
 */
export const HistoryPage = () => {
  const { t } = useTranslation();
  const { loadAllHistory, searchAllHistory } = useQueryHistory();

  const [query, setQuery] = useState("");
  const [recent, setRecent] = useState<QueryHistoryEntry[]>([]);
  const [results, setResults] = useState<QueryHistoryEntry[] | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSearching, setIsSearching] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      setIsLoading(true);
      try {
        const entries = await loadAllHistory();
        if (!cancelled) setRecent(entries);
      } finally {
        if (!cancelled) setIsLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [loadAllHistory]);

  const runSearch = useCallback(
    async (value: string) => {
      const trimmed = value.trim();
      if (!trimmed) {
        setResults(null);
        return;
      }
      setIsSearching(true);
      try {
        setResults(await searchAllHistory(trimmed, 500));
      } finally {
        setIsSearching(false);
      }
    },
    [searchAllHistory],
  );

  // Debounce so typing does not stream a file scan per keystroke.
  useEffect(() => {
    const id = setTimeout(() => void runSearch(query), 250);
    return () => clearTimeout(id);
  }, [query, runSearch]);

  const shown = results ?? recent;
  const isSearchMode = results !== null;

  const summary = useMemo(
    () =>
      isSearchMode
        ? t("history.searchSummary", { count: shown.length })
        : t("history.recentSummary", { count: shown.length }),
    [isSearchMode, shown.length, t],
  );

  return (
    <div className="flex flex-col h-full min-h-0">
      <div className="p-4 border-b border-default shrink-0">
        <div className="flex items-center gap-2 mb-1">
          <History size={16} className="text-secondary" />
          <h1 className="text-lg font-semibold text-primary">
            {t("history.title")}
          </h1>
        </div>
        <p className="text-xs text-muted">{summary}</p>

        <div className="relative mt-3">
          <Search
            size={14}
            className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted pointer-events-none"
          />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("history.searchPlaceholder")}
            className="w-full pl-8 pr-8 py-2 bg-surface-secondary border border-default rounded text-sm text-primary placeholder:text-muted focus:outline-none focus:ring-2 focus:ring-blue-500/40"
          />
          {query && (
            <button
              onClick={() => setQuery("")}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-muted hover:text-primary"
              title={t("history.clearSearch")}
            >
              <X size={14} />
            </button>
          )}
        </div>
      </div>

      <div className="flex-1 min-h-0 overflow-y-auto">
        {(isLoading || isSearching) && (
          <div className="flex items-center gap-2 px-4 py-3 text-xs text-muted">
            <Loader2 size={12} className="animate-spin" />
            {t("history.loading")}
          </div>
        )}
        {!isLoading && !isSearching && shown.length === 0 && (
          <div className="flex flex-col items-center justify-center py-16 text-secondary">
            <History size={40} className="mb-3 opacity-20" />
            <p className="text-sm">
              {isSearchMode ? t("history.noMatches") : t("history.empty")}
            </p>
          </div>
        )}
        <ul className="divide-y divide-default">
          {shown.map((entry) => (
            <li key={entry.id} className="px-4 py-3 hover:bg-surface-secondary">
              <div className="flex items-center gap-2 mb-1 text-[11px] text-muted">
                <span
                  className={
                    entry.status === "error"
                      ? "text-red-400 font-medium"
                      : "text-emerald-400 font-medium"
                  }
                >
                  {entry.status === "error"
                    ? t("history.statusError")
                    : t("history.statusSuccess")}
                </span>
                <span>{new Date(entry.executedAt).toLocaleString()}</span>
                {entry.connectionName && (
                  <span className="text-secondary">
                    · {entry.connectionName}
                  </span>
                )}
                {entry.database && <span>· {entry.database}</span>}
                {entry.executionTimeMs != null && (
                  <span>· {formatDuration(entry.executionTimeMs)}</span>
                )}
                {entry.rowsAffected != null && (
                  <span>· {t("history.rows", { count: entry.rowsAffected })}</span>
                )}
              </div>
              <pre className="text-xs font-mono text-secondary whitespace-pre-wrap break-words max-h-32 overflow-auto">
                {entry.sql}
              </pre>
              {entry.error && (
                <div className="mt-1 flex items-start gap-1.5 text-[11px] text-red-400">
                  <AlertCircle size={12} className="mt-0.5 shrink-0" />
                  <span className="break-words">{entry.error}</span>
                </div>
              )}
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
};
