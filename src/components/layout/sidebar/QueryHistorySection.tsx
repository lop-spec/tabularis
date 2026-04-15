import { useState, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { History, Search, CircleCheck, CircleX, Trash2 } from "lucide-react";
import { groupByDate, formatHistoryTime } from "../../../utils/dateGroups";
import type { QueryHistoryEntry } from "../../../types/queryHistory";

interface QueryHistorySectionProps {
  entries: QueryHistoryEntry[];
  onDoubleClick: (entry: QueryHistoryEntry) => void;
  onContextMenu: (
    e: React.MouseEvent,
    entry: QueryHistoryEntry,
  ) => void;
  onClearAll: () => void;
}

export function QueryHistorySection({
  entries,
  onDoubleClick,
  onContextMenu,
  onClearAll,
}: QueryHistorySectionProps) {
  const { t } = useTranslation();
  const [search, setSearch] = useState("");

  const filteredEntries = useMemo(() => {
    if (!search.trim()) return entries;
    const lower = search.toLowerCase();
    return entries.filter((e) => e.sql.toLowerCase().includes(lower));
  }, [entries, search]);

  const groupedEntries = useMemo(
    () => groupByDate(filteredEntries, (e) => e.executedAt),
    [filteredEntries],
  );

  const truncateSql = (sql: string, maxLen = 60): string => {
    const firstLine = sql.split("\n")[0].trim();
    if (firstLine.length <= maxLen) return firstLine;
    return firstLine.slice(0, maxLen) + "...";
  };

  const formatDuration = (ms: number | null): string => {
    if (ms === null) return "";
    if (ms < 1000) return `${Math.round(ms)}ms`;
    return `${(ms / 1000).toFixed(1)}s`;
  };

  if (entries.length === 0) {
    return (
      <div className="text-center p-4 text-xs text-muted italic">
        {t("sidebar.noQueryHistory")}
      </div>
    );
  }

  return (
    <div>
      {/* Header with search and clear */}
      <div className="px-2 pb-1.5 flex items-center gap-1">
        <div className="relative flex-1">
          <Search
            size={12}
            className="absolute left-2 top-1/2 -translate-y-1/2 text-muted"
          />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder={t("sidebar.searchHistory")}
            className="w-full pl-6 pr-2 py-1 text-xs bg-surface-secondary border border-default rounded text-primary placeholder:text-muted focus:outline-none focus:border-blue-500/50"
          />
        </div>
        <button
          onClick={onClearAll}
          className="p-1 text-muted hover:text-red-500 transition-colors shrink-0"
          title={t("sidebar.clearAllHistory")}
        >
          <Trash2 size={13} />
        </button>
      </div>

      {/* Grouped entries */}
      {groupedEntries.length === 0 ? (
        <div className="text-center p-2 text-xs text-muted italic">
          {t("sidebar.noQueryHistory")}
        </div>
      ) : (
        groupedEntries.map(([groupKey, items]) => (
          <div key={groupKey}>
            <div className="px-3 py-1 text-[10px] font-semibold uppercase text-muted tracking-wider">
              {t(`sidebar.${groupKey}`)}
            </div>
            {items.map((entry) => (
              <div
                key={entry.id}
                onDoubleClick={() => onDoubleClick(entry)}
                onContextMenu={(e) => onContextMenu(e, entry)}
                className="flex items-center gap-2 pl-3 pr-4 py-1.5 text-sm text-secondary hover:bg-surface-secondary hover:text-primary cursor-pointer group transition-colors"
                title={entry.sql}
              >
                <div className="relative shrink-0">
                  <History size={14} className="text-muted" />
                  {entry.status === "success" ? (
                    <CircleCheck
                      size={8}
                      className="absolute -bottom-0.5 -right-0.5 text-green-500"
                    />
                  ) : (
                    <CircleX
                      size={8}
                      className="absolute -bottom-0.5 -right-0.5 text-red-500"
                    />
                  )}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="truncate text-xs">
                    {truncateSql(entry.sql)}
                  </div>
                </div>
                <div className="shrink-0 text-[10px] text-muted flex items-center gap-1">
                  {entry.executionTimeMs !== null && (
                    <span>{formatDuration(entry.executionTimeMs)}</span>
                  )}
                  <span>{formatHistoryTime(entry.executedAt)}</span>
                </div>
              </div>
            ))}
          </div>
        ))
      )}
    </div>
  );
}
