import { invoke } from "@tauri-apps/api/core";
import {
  AlertTriangle,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Clock3,
  Copy,
  Database,
  FileText,
  History,
  Loader2,
  RefreshCw,
  RotateCcw,
  Search,
  ShieldCheck,
  X,
  XCircle,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useLocation } from "react-router-dom";
import { useDatabase } from "../hooks/useDatabase";
import { useQueryHistory } from "../hooks/useQueryHistory";
import type { QueryHistoryEntry } from "../types/queryHistory";
import { formatDuration } from "../utils/formatTime";

interface RecoveryStatementSummary {
  id: string;
  index: number;
  executedAt: string;
  sql: string;
  category: "ddl" | "dml" | "unprotected";
  operation: string;
  schema?: string;
  table?: string;
  affectedColumns: string[];
  condition?: string;
  rowCount: number;
  exact: boolean;
}

interface RecoveryRunSummary {
  runId: string;
  shortId: string;
  startedAt: string;
  finishedAt?: string;
  status: string;
  connectionId: string;
  connectionName: string;
  database: string;
  statementCount: number;
  statements: RecoveryStatementSummary[];
}

interface RecoveryCompareResponse {
  outputPath: string;
  sql: string;
  generatedSteps: number;
  unchangedRows: number;
  conflicts: string[];
  exact: boolean;
  targetInstance: string;
  backupInstance: string;
}

const inputClass =
  "w-full rounded-lg border border-default bg-base px-3 py-1.5 text-sm text-primary outline-none transition-colors placeholder:text-muted focus:border-blue-500/60";

function localDateTimeInput(date: Date): string {
  const local = new Date(date.getTime() - date.getTimezoneOffset() * 60_000);
  return local.toISOString().slice(0, 16);
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function displayTime(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}

/**
 * Execution history and rollback in one place.
 *
 * One list, one detail/action pane. The "All executions" view is the complete
 * cross-connection statement log; the "Recoverable changes" view lists the
 * protected write batches whose recorded row images can produce rollback SQL
 * offline — click statements, then generate.
 */
export function HistoryRecoveryPage() {
  const { i18n } = useTranslation();
  const zh = i18n.language.toLowerCase().startsWith("zh");
  const location = useLocation();
  const { activeConnectionId, connections } = useDatabase();
  const { loadAllHistory, searchAllHistory } = useQueryHistory();

  const labels = zh
    ? {
        title: "历史与恢复",
        tabHistory: "全部执行",
        tabRecovery: "可恢复变更",
        searchHistory: "搜索全部执行历史",
        searchRecovery: "搜索可恢复变更（SQL、库表、连接、Run ID）",
        clearSearch: "清除搜索",
        refresh: "刷新",
        activeOnly: "仅当前连接",
        allConnections: "全部连接",
        from: "开始时间",
        to: "结束时间",
        loading: "加载中…",
        historyEmpty: "还没有执行记录",
        historyNoMatches: "没有匹配的执行记录",
        historyRecent: (count: number) => `最近 ${count} 条执行`,
        historyFound: (count: number) => `匹配 ${count} 条执行`,
        statusSuccess: "成功",
        statusError: "失败",
        detailEmpty: "选择一条记录查看详情",
        copySql: "复制 SQL",
        copied: "已复制",
        runBatch: "批次",
        duration: "耗时",
        rows: (count: number) => `${count} 行`,
        exact: "精确",
        inexact: "不可精确",
        recoveryEmpty: "这个时间范围内没有可恢复变更",
        recoveryEmptyHint: "受保护执行的 MySQL / MariaDB 写操作会自动记录在这里。",
        selectedSummary: (statements: number, runs: number) =>
          `已选 ${statements} 条语句 · ${runs} 个批次`,
        nothingSelected: "点击左侧语句选择要回滚的变更",
        clearSelection: "清空选择",
        generateOffline: "生成回滚 SQL",
        generateOfflineHint: "从记录的行镜像离线生成，无需连接任何实例。",
        offlineNeedsExact: "所选包含不可精确恢复的语句，请改用备份实例对比。",
        advancedBackup: "备份实例对比（用于不可精确的记录）",
        backupConnection: "已保存连接",
        pickConnection: "请选择 MySQL / MariaDB 连接",
        noConnections: "暂无可用的 MySQL / MariaDB 连接",
        testConnection: "测试连接",
        tested: "连接成功",
        generateBackup: "只读对比并生成",
        resultTitle: "回滚 SQL",
        resultHint: "只生成文件，不会自动执行；默认以 ROLLBACK 结尾，复核后改 COMMIT。",
        steps: "步骤",
        unchanged: "无需恢复",
        conflicts: "冲突",
        target: "目标",
        source: "来源",
        file: "文件",
        oneConnection: "一次只能回滚同一个目标连接的记录。",
        pickFirst: "请先选择要回滚的语句。",
        pickBackup: "请先选择备份连接。",
      }
    : {
        title: "History & Recovery",
        tabHistory: "All executions",
        tabRecovery: "Recoverable changes",
        searchHistory: "Search all execution history",
        searchRecovery: "Search recoverable changes (SQL, objects, connections, Run ID)",
        clearSearch: "Clear search",
        refresh: "Refresh",
        activeOnly: "Active connection only",
        allConnections: "All connections",
        from: "From",
        to: "To",
        loading: "Loading…",
        historyEmpty: "No executions yet",
        historyNoMatches: "No matching executions",
        historyRecent: (count: number) => `${count} recent executions`,
        historyFound: (count: number) => `${count} matching executions`,
        statusSuccess: "OK",
        statusError: "Failed",
        detailEmpty: "Select an entry to see its details",
        copySql: "Copy SQL",
        copied: "Copied",
        runBatch: "Batch",
        duration: "Duration",
        rows: (count: number) => `${count} rows`,
        exact: "Exact",
        inexact: "Inexact",
        recoveryEmpty: "No recoverable changes in this time range",
        recoveryEmptyHint:
          "Protected MySQL / MariaDB writes are recorded here automatically.",
        selectedSummary: (statements: number, runs: number) =>
          `${statements} statement${statements === 1 ? "" : "s"} selected · ${runs} batch${runs === 1 ? "" : "es"}`,
        nothingSelected: "Click statements on the left to pick changes to roll back",
        clearSelection: "Clear selection",
        generateOffline: "Generate rollback SQL",
        generateOfflineHint:
          "Built offline from recorded row images; no instance connection needed.",
        offlineNeedsExact:
          "The selection contains inexact statements; use the backup comparison instead.",
        advancedBackup: "Backup-instance comparison (for inexact records)",
        backupConnection: "Saved connection",
        pickConnection: "Select a MySQL / MariaDB connection",
        noConnections: "No saved MySQL / MariaDB connections",
        testConnection: "Test connection",
        tested: "Connected",
        generateBackup: "Compare read-only and generate",
        resultTitle: "Rollback SQL",
        resultHint:
          "A file is generated, never executed; DML ends in ROLLBACK — review, then COMMIT.",
        steps: "Steps",
        unchanged: "No change",
        conflicts: "Conflicts",
        target: "Target",
        source: "Source",
        file: "File",
        oneConnection: "A rollback selection can only target one connection.",
        pickFirst: "Select the statements to roll back first.",
        pickBackup: "Select a backup connection first.",
      };

  const [tab, setTab] = useState<"history" | "recovery">(
    location.pathname === "/recovery" ? "recovery" : "history",
  );
  const [query, setQuery] = useState("");
  const [debouncedQuery, setDebouncedQuery] = useState("");
  useEffect(() => {
    const id = setTimeout(() => setDebouncedQuery(query.trim()), 300);
    return () => clearTimeout(id);
  }, [query]);

  // ---------------- All executions ----------------
  const [historyEntries, setHistoryEntries] = useState<QueryHistoryEntry[]>([]);
  const [historyResults, setHistoryResults] = useState<QueryHistoryEntry[] | null>(null);
  const [historyLoading, setHistoryLoading] = useState(true);
  const [historyDetail, setHistoryDetail] = useState<QueryHistoryEntry | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      setHistoryLoading(true);
      try {
        const entries = await loadAllHistory();
        if (!cancelled) setHistoryEntries(entries);
      } finally {
        if (!cancelled) setHistoryLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [loadAllHistory]);

  useEffect(() => {
    if (tab !== "history") return;
    if (!debouncedQuery) {
      setHistoryResults(null);
      return;
    }
    let cancelled = false;
    void (async () => {
      setHistoryLoading(true);
      try {
        const entries = await searchAllHistory(debouncedQuery, 500);
        if (!cancelled) setHistoryResults(entries);
      } finally {
        if (!cancelled) setHistoryLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [tab, debouncedQuery, searchAllHistory]);

  const shownHistory = historyResults ?? historyEntries;

  // ---------------- Recoverable changes ----------------
  const now = useMemo(() => new Date(), []);
  const [startedAfter, setStartedAfter] = useState(() =>
    localDateTimeInput(new Date(now.getTime() - 7 * 24 * 60 * 60 * 1_000)),
  );
  const [startedBefore, setStartedBefore] = useState(() => localDateTimeInput(now));
  const [activeOnly, setActiveOnly] = useState(true);
  const [runs, setRuns] = useState<RecoveryRunSummary[]>([]);
  const [runsLoading, setRunsLoading] = useState(false);
  const [collapsedRuns, setCollapsedRuns] = useState<Set<string>>(new Set());
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const [generating, setGenerating] = useState(false);
  const [copied, setCopied] = useState(false);
  const [result, setResult] = useState<RecoveryCompareResponse | null>(null);
  const [backupOpen, setBackupOpen] = useState(false);
  const [backupConnectionId, setBackupConnectionId] = useState("");
  const [testState, setTestState] = useState<"idle" | "testing" | "ok">("idle");

  const backupConnections = useMemo(
    () =>
      connections.filter((connection) =>
        ["mysql", "mariadb"].includes(connection.params.driver.toLowerCase()),
      ),
    [connections],
  );

  const loadRuns = useCallback(async () => {
    setRunsLoading(true);
    setError(null);
    try {
      const response = await invoke<RecoveryRunSummary[]>("list_recovery_runs", {
        connectionId: activeOnly ? activeConnectionId : null,
        startedAfter:
          debouncedQuery || !startedAfter ? null : new Date(startedAfter).toISOString(),
        startedBefore:
          debouncedQuery || !startedBefore ? null : new Date(startedBefore).toISOString(),
        query: debouncedQuery || null,
      });
      setRuns(response);
      const available = new Set(
        response.flatMap((run) => run.statements.map((statement) => statement.id)),
      );
      setSelected((current) => new Set([...current].filter((id) => available.has(id))));
    } catch (loadError) {
      setError(errorText(loadError));
    } finally {
      setRunsLoading(false);
    }
  }, [activeConnectionId, activeOnly, debouncedQuery, startedAfter, startedBefore]);

  useEffect(() => {
    if (tab === "recovery") void loadRuns();
  }, [tab, loadRuns]);

  const selectedRuns = useMemo(
    () =>
      runs.filter((run) => run.statements.some((statement) => selected.has(statement.id))),
    [runs, selected],
  );
  const selectedConnectionIds = useMemo(
    () => new Set(selectedRuns.map((run) => run.connectionId)),
    [selectedRuns],
  );
  const targetConnectionId = selectedRuns[0]?.connectionId ?? null;
  const selectedAllExact = useMemo(
    () =>
      selectedRuns.every((run) =>
        run.statements.every(
          (statement) => !selected.has(statement.id) || statement.exact,
        ),
      ),
    [selectedRuns, selected],
  );

  const toggleStatement = (statementId: string) => {
    setError(null);
    setResult(null);
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(statementId)) next.delete(statementId);
      else next.add(statementId);
      return next;
    });
  };

  const toggleRun = (run: RecoveryRunSummary) => {
    setError(null);
    setResult(null);
    setSelected((current) => {
      const next = new Set(current);
      const allSelected = run.statements.every((statement) => next.has(statement.id));
      run.statements.forEach((statement) => {
        if (allSelected) next.delete(statement.id);
        else next.add(statement.id);
      });
      return next;
    });
  };

  const runGenerate = async (mode: "offline" | "backup") => {
    if (!targetConnectionId || selected.size === 0) {
      setError(labels.pickFirst);
      return;
    }
    if (selectedConnectionIds.size !== 1) {
      setError(labels.oneConnection);
      return;
    }
    if (mode === "backup" && !backupConnectionId) {
      setError(labels.pickBackup);
      return;
    }
    setGenerating(true);
    setError(null);
    setResult(null);
    try {
      const selection = {
        runIds: selectedRuns.map((run) => run.runId),
        statementIds: [...selected],
      };
      const response = await invoke<RecoveryCompareResponse>(
        mode === "offline" ? "generate_offline_recovery_sql" : "generate_recovery_sql",
        mode === "offline"
          ? { connectionId: targetConnectionId, selection }
          : { connectionId: targetConnectionId, selection, backupConnectionId },
      );
      setResult(response);
    } catch (generateError) {
      setError(errorText(generateError));
    } finally {
      setGenerating(false);
    }
  };

  const testBackup = async () => {
    if (!backupConnectionId) {
      setError(labels.pickBackup);
      return;
    }
    setTestState("testing");
    setError(null);
    try {
      await invoke<string>("test_saved_connection", { connectionId: backupConnectionId });
      setTestState("ok");
    } catch (testError) {
      setTestState("idle");
      setError(errorText(testError));
    }
  };

  const copyResult = () => {
    if (!result) return;
    void navigator.clipboard.writeText(result.sql);
    setCopied(true);
    setTimeout(() => setCopied(false), 1_500);
  };

  const statusDot = (ok: boolean) => (
    <span
      className={`inline-block h-1.5 w-1.5 shrink-0 rounded-full ${
        ok ? "bg-emerald-400" : "bg-red-400"
      }`}
    />
  );

  return (
    <div className="flex h-full min-h-0 flex-col bg-base">
      {/* Toolbar */}
      <header className="shrink-0 space-y-3 border-b border-default px-5 py-3">
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <History size={17} className="text-secondary" />
            <h1 className="text-base font-semibold text-primary">{labels.title}</h1>
          </div>
          <div
            role="tablist"
            className="flex overflow-hidden rounded-lg border border-default text-xs"
          >
            <button
              role="tab"
              aria-selected={tab === "history"}
              onClick={() => setTab("history")}
              className={`px-3 py-1.5 transition-colors ${
                tab === "history"
                  ? "bg-surface-secondary font-medium text-primary"
                  : "text-secondary hover:text-primary"
              }`}
            >
              {labels.tabHistory}
            </button>
            <button
              role="tab"
              aria-selected={tab === "recovery"}
              onClick={() => setTab("recovery")}
              className={`border-l border-default px-3 py-1.5 transition-colors ${
                tab === "recovery"
                  ? "bg-surface-secondary font-medium text-primary"
                  : "text-secondary hover:text-primary"
              }`}
            >
              {labels.tabRecovery}
            </button>
          </div>
          <div className="relative min-w-0 flex-1">
            <Search
              size={14}
              className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted"
            />
            <input
              aria-label={tab === "history" ? labels.searchHistory : labels.searchRecovery}
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={
                tab === "history" ? labels.searchHistory : labels.searchRecovery
              }
              className={`${inputClass} pl-9 pr-8`}
            />
            {query ? (
              <button
                aria-label={labels.clearSearch}
                onClick={() => setQuery("")}
                className="absolute right-2 top-1/2 -translate-y-1/2 rounded p-0.5 text-muted hover:text-secondary"
              >
                <X size={13} />
              </button>
            ) : null}
          </div>
          {tab === "recovery" && (
            <button
              type="button"
              onClick={() => void loadRuns()}
              title={labels.refresh}
              className="inline-flex items-center gap-1.5 rounded-lg border border-default px-2.5 py-1.5 text-xs text-secondary transition-colors hover:bg-surface-secondary"
            >
              <RefreshCw size={13} className={runsLoading ? "animate-spin" : ""} />
              {labels.refresh}
            </button>
          )}
        </div>

        {tab === "recovery" && (
          <div className="flex flex-wrap items-center gap-2 text-xs">
            <button
              type="button"
              onClick={() => setActiveOnly((value) => !value)}
              className={`rounded-lg border px-2.5 py-1 transition-colors ${
                activeOnly
                  ? "border-blue-500/40 bg-blue-500/10 text-blue-300"
                  : "border-default text-secondary hover:bg-surface-secondary"
              }`}
            >
              {activeOnly ? labels.activeOnly : labels.allConnections}
            </button>
            <label className="flex items-center gap-1.5 text-muted">
              {labels.from}
              <input
                className={`${inputClass} w-auto py-1 text-xs`}
                type="datetime-local"
                disabled={Boolean(debouncedQuery)}
                value={startedAfter}
                onChange={(event) => setStartedAfter(event.target.value)}
              />
            </label>
            <label className="flex items-center gap-1.5 text-muted">
              {labels.to}
              <input
                className={`${inputClass} w-auto py-1 text-xs`}
                type="datetime-local"
                disabled={Boolean(debouncedQuery)}
                value={startedBefore}
                onChange={(event) => setStartedBefore(event.target.value)}
              />
            </label>
          </div>
        )}
      </header>

      <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_minmax(340px,420px)]">
        {/* List */}
        <section className="min-h-0 overflow-y-auto border-r border-default">
          {tab === "history" ? (
            <>
              {historyLoading && (
                <div className="flex items-center gap-2 px-5 py-3 text-xs text-muted">
                  <Loader2 size={12} className="animate-spin" />
                  {labels.loading}
                </div>
              )}
              {!historyLoading && shownHistory.length === 0 && (
                <div className="flex flex-col items-center justify-center py-20 text-center">
                  <History size={32} className="mb-3 text-muted opacity-40" />
                  <p className="text-sm text-secondary">
                    {historyResults ? labels.historyNoMatches : labels.historyEmpty}
                  </p>
                </div>
              )}
              {shownHistory.length > 0 && (
                <>
                  <p className="px-5 pb-1 pt-3 text-[11px] text-muted">
                    {historyResults
                      ? labels.historyFound(shownHistory.length)
                      : labels.historyRecent(shownHistory.length)}
                  </p>
                  <ul className="divide-y divide-default">
                    {shownHistory.map((entry) => (
                      <li key={entry.id}>
                        <button
                          type="button"
                          onClick={() => setHistoryDetail(entry)}
                          className={`w-full px-5 py-2.5 text-left transition-colors hover:bg-surface-secondary/60 ${
                            historyDetail?.id === entry.id ? "bg-surface-secondary/60" : ""
                          }`}
                        >
                          <div className="flex items-center gap-2 text-[11px] text-muted">
                            {statusDot(entry.status !== "error")}
                            <span className="tabular-nums">
                              {displayTime(entry.executedAt)}
                            </span>
                            {entry.connectionName && (
                              <span className="truncate text-secondary">
                                {entry.connectionName}
                              </span>
                            )}
                            {entry.database && <span>· {entry.database}</span>}
                            {entry.executionTimeMs != null && (
                              <span className="ml-auto tabular-nums">
                                {formatDuration(entry.executionTimeMs)}
                              </span>
                            )}
                          </div>
                          <pre className="mt-1 line-clamp-2 whitespace-pre-wrap break-all font-mono text-xs text-secondary">
                            {entry.sql}
                          </pre>
                        </button>
                      </li>
                    ))}
                  </ul>
                </>
              )}
            </>
          ) : (
            <>
              {runsLoading && (
                <div className="flex items-center gap-2 px-5 py-3 text-xs text-muted">
                  <Loader2 size={12} className="animate-spin" />
                  {labels.loading}
                </div>
              )}
              {!runsLoading && runs.length === 0 && (
                <div className="flex flex-col items-center justify-center py-20 text-center">
                  <Clock3 size={32} className="mb-3 text-muted opacity-40" />
                  <p className="text-sm text-secondary">{labels.recoveryEmpty}</p>
                  <p className="mt-1 max-w-sm text-xs text-muted">
                    {labels.recoveryEmptyHint}
                  </p>
                </div>
              )}
              {runs.map((run) => {
                const collapsed = collapsedRuns.has(run.runId);
                const allSelected =
                  run.statements.length > 0 &&
                  run.statements.every((statement) => selected.has(statement.id));
                return (
                  <div key={run.runId} className="border-b border-default">
                    <div className="flex items-center gap-2 bg-elevated px-5 py-2">
                      <button
                        type="button"
                        aria-label={`${labels.runBatch} ${run.shortId}`}
                        onClick={() =>
                          setCollapsedRuns((current) => {
                            const next = new Set(current);
                            if (next.has(run.runId)) next.delete(run.runId);
                            else next.add(run.runId);
                            return next;
                          })
                        }
                        className="text-muted hover:text-secondary"
                      >
                        {collapsed ? <ChevronRight size={14} /> : <ChevronDown size={14} />}
                      </button>
                      <input
                        type="checkbox"
                        aria-label={`${labels.runBatch} ${run.shortId} select`}
                        checked={allSelected}
                        onChange={() => toggleRun(run)}
                        className="h-3.5 w-3.5 accent-blue-500"
                      />
                      <span className="font-mono text-[11px] text-blue-300">
                        {run.shortId}
                      </span>
                      <span className="truncate text-xs text-secondary">
                        {run.connectionName} · {run.database}
                      </span>
                      <span className="ml-auto shrink-0 text-[11px] tabular-nums text-muted">
                        {displayTime(run.startedAt)}
                      </span>
                    </div>
                    {!collapsed &&
                      run.statements.map((statement) => (
                        <button
                          type="button"
                          key={statement.id}
                          onClick={() => toggleStatement(statement.id)}
                          className={`flex w-full items-start gap-2 px-5 py-2 text-left transition-colors hover:bg-surface-secondary/60 ${
                            selected.has(statement.id) ? "bg-blue-500/5" : ""
                          }`}
                        >
                          <input
                            type="checkbox"
                            readOnly
                            tabIndex={-1}
                            checked={selected.has(statement.id)}
                            className="mt-0.5 h-3.5 w-3.5 shrink-0 accent-blue-500"
                          />
                          <div className="min-w-0 flex-1">
                            <div className="flex items-center gap-2 text-[11px]">
                              <span className="font-mono uppercase text-secondary">
                                {statement.operation}
                              </span>
                              {statement.table && (
                                <span className="truncate text-muted">
                                  {statement.schema ? `${statement.schema}.` : ""}
                                  {statement.table}
                                </span>
                              )}
                              <span className="text-muted">
                                {labels.rows(statement.rowCount)}
                              </span>
                              <span
                                className={`ml-auto shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium ${
                                  statement.exact
                                    ? "bg-emerald-500/10 text-emerald-400"
                                    : "bg-amber-500/10 text-amber-400"
                                }`}
                              >
                                {statement.exact ? labels.exact : labels.inexact}
                              </span>
                            </div>
                            <pre className="mt-0.5 line-clamp-2 whitespace-pre-wrap break-all font-mono text-xs text-secondary">
                              {statement.sql}
                            </pre>
                          </div>
                        </button>
                      ))}
                  </div>
                );
              })}
            </>
          )}
        </section>

        {/* Detail / actions */}
        <aside className="min-h-0 overflow-y-auto">
          {tab === "history" ? (
            historyDetail ? (
              <div className="p-5">
                <div className="flex items-center gap-2 text-xs">
                  {historyDetail.status === "error" ? (
                    <XCircle size={14} className="text-red-400" />
                  ) : (
                    <CheckCircle2 size={14} className="text-emerald-400" />
                  )}
                  <span className="font-medium text-primary">
                    {historyDetail.status === "error"
                      ? labels.statusError
                      : labels.statusSuccess}
                  </span>
                  <span className="ml-auto text-muted tabular-nums">
                    {displayTime(historyDetail.executedAt)}
                  </span>
                </div>
                <dl className="mt-3 space-y-1 text-xs text-muted">
                  {historyDetail.connectionName && (
                    <div className="flex gap-2">
                      <dt className="w-16 shrink-0">{labels.source}</dt>
                      <dd className="text-secondary">
                        {historyDetail.connectionName}
                        {historyDetail.database ? ` · ${historyDetail.database}` : ""}
                      </dd>
                    </div>
                  )}
                  {historyDetail.executionTimeMs != null && (
                    <div className="flex gap-2">
                      <dt className="w-16 shrink-0">{labels.duration}</dt>
                      <dd className="tabular-nums text-secondary">
                        {formatDuration(historyDetail.executionTimeMs)}
                      </dd>
                    </div>
                  )}
                </dl>
                <div className="mt-3 flex items-center justify-between">
                  <span className="text-[11px] font-medium uppercase tracking-wide text-muted">
                    SQL
                  </span>
                  <button
                    type="button"
                    onClick={() => {
                      void navigator.clipboard.writeText(historyDetail.sql);
                      setCopied(true);
                      setTimeout(() => setCopied(false), 1_500);
                    }}
                    className="inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[11px] text-secondary hover:bg-surface-secondary"
                  >
                    <Copy size={11} />
                    {copied ? labels.copied : labels.copySql}
                  </button>
                </div>
                <pre className="mt-1 max-h-[50vh] overflow-auto rounded-lg border border-default bg-elevated p-3 font-mono text-xs text-secondary whitespace-pre-wrap break-all">
                  {historyDetail.sql}
                </pre>
                {historyDetail.error && (
                  <div className="mt-3 flex gap-2 rounded-lg border border-red-500/25 bg-red-500/5 p-3 text-xs text-red-300">
                    <AlertTriangle size={14} className="mt-0.5 shrink-0" />
                    <span className="break-words">{historyDetail.error}</span>
                  </div>
                )}
              </div>
            ) : (
              <div className="flex h-full flex-col items-center justify-center p-8 text-center">
                <FileText size={28} className="mb-3 text-muted opacity-40" />
                <p className="text-xs text-muted">{labels.detailEmpty}</p>
              </div>
            )
          ) : (
            <div className="flex min-h-full flex-col p-5">
              {/* Selection + primary action */}
              <p className="text-xs text-secondary">
                {selected.size > 0
                  ? labels.selectedSummary(selected.size, selectedRuns.length)
                  : labels.nothingSelected}
              </p>
              {selected.size > 0 && (
                <button
                  type="button"
                  onClick={() => {
                    setSelected(new Set());
                    setResult(null);
                  }}
                  className="mt-1 self-start text-[11px] text-muted underline-offset-2 hover:text-secondary hover:underline"
                >
                  {labels.clearSelection}
                </button>
              )}

              <button
                type="button"
                disabled={
                  generating ||
                  selected.size === 0 ||
                  selectedConnectionIds.size !== 1 ||
                  !selectedAllExact
                }
                title={!selectedAllExact ? labels.offlineNeedsExact : undefined}
                onClick={() => void runGenerate("offline")}
                className="mt-4 inline-flex w-full items-center justify-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-blue-500 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {generating ? (
                  <Loader2 className="animate-spin" size={15} />
                ) : (
                  <RotateCcw size={15} />
                )}
                {labels.generateOffline}
              </button>
              <p className="mt-1.5 text-[11px] text-muted">{labels.generateOfflineHint}</p>
              {!selectedAllExact && selected.size > 0 && (
                <p className="mt-1 text-[11px] text-amber-400">
                  {labels.offlineNeedsExact}
                </p>
              )}

              {/* Advanced: backup-instance comparison */}
              <div className="mt-4 rounded-lg border border-default">
                <button
                  type="button"
                  onClick={() => setBackupOpen((value) => !value)}
                  className="flex w-full items-center gap-2 px-3 py-2 text-xs text-secondary hover:bg-surface-secondary/60"
                >
                  {backupOpen ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
                  <Database size={13} className="text-muted" />
                  {labels.advancedBackup}
                </button>
                {backupOpen && (
                  <div className="space-y-3 border-t border-default p-3">
                    <label className="block space-y-1">
                      <span className="text-[11px] text-muted">
                        {labels.backupConnection}
                      </span>
                      <select
                        aria-label={labels.backupConnection}
                        className={inputClass}
                        value={backupConnectionId}
                        onChange={(event) => {
                          setBackupConnectionId(event.target.value);
                          setTestState("idle");
                        }}
                      >
                        <option value="">
                          {backupConnections.length > 0
                            ? labels.pickConnection
                            : labels.noConnections}
                        </option>
                        {backupConnections.map((connection) => (
                          <option key={connection.id} value={connection.id}>
                            {connection.name} · {connection.params.driver.toUpperCase()}
                          </option>
                        ))}
                      </select>
                    </label>
                    <div className="flex gap-2">
                      <button
                        type="button"
                        disabled={testState === "testing" || !backupConnectionId}
                        onClick={() => void testBackup()}
                        className="inline-flex flex-1 items-center justify-center gap-1.5 rounded-lg border border-default px-3 py-1.5 text-xs text-secondary transition-colors hover:bg-surface-secondary disabled:cursor-not-allowed disabled:opacity-50"
                      >
                        {testState === "testing" ? (
                          <Loader2 className="animate-spin" size={13} />
                        ) : testState === "ok" ? (
                          <CheckCircle2 className="text-emerald-400" size={13} />
                        ) : (
                          <Database size={13} />
                        )}
                        {testState === "ok" ? labels.tested : labels.testConnection}
                      </button>
                      <button
                        type="button"
                        disabled={
                          generating ||
                          selected.size === 0 ||
                          selectedConnectionIds.size !== 1 ||
                          !backupConnectionId
                        }
                        onClick={() => void runGenerate("backup")}
                        className="inline-flex flex-1 items-center justify-center gap-1.5 rounded-lg border border-default px-3 py-1.5 text-xs text-secondary transition-colors hover:bg-surface-secondary disabled:cursor-not-allowed disabled:opacity-50"
                      >
                        <ShieldCheck size={13} />
                        {labels.generateBackup}
                      </button>
                    </div>
                  </div>
                )}
              </div>

              {error && (
                <div className="mt-4 flex gap-2 rounded-lg border border-red-500/25 bg-red-500/5 p-3 text-xs text-red-300">
                  <AlertTriangle size={14} className="mt-0.5 shrink-0" />
                  <span className="break-words">{error}</span>
                </div>
              )}

              {result && (
                <div className="mt-4 overflow-hidden rounded-lg border border-default">
                  <div className="flex items-center gap-2 border-b border-default bg-elevated px-3 py-2">
                    <FileText size={14} className="text-blue-400" />
                    <span className="text-xs font-semibold text-primary">
                      {labels.resultTitle}
                    </span>
                    <button
                      type="button"
                      onClick={copyResult}
                      className="ml-auto inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[11px] text-secondary hover:bg-surface-secondary"
                    >
                      <Copy size={11} />
                      {copied ? labels.copied : labels.copySql}
                    </button>
                  </div>
                  <div className="flex flex-wrap gap-x-4 gap-y-1 px-3 py-2 text-[11px] text-muted">
                    <span>
                      {labels.steps}:{" "}
                      <span className="tabular-nums text-secondary">
                        {result.generatedSteps}
                      </span>
                    </span>
                    <span>
                      {labels.unchanged}:{" "}
                      <span className="tabular-nums text-secondary">
                        {result.unchangedRows}
                      </span>
                    </span>
                    <span>
                      {labels.conflicts}:{" "}
                      <span
                        className={`tabular-nums ${
                          result.conflicts.length > 0 ? "text-amber-400" : "text-secondary"
                        }`}
                      >
                        {result.conflicts.length}
                      </span>
                    </span>
                    <span
                      className="w-full truncate"
                      title={`${result.targetInstance} · ${result.backupInstance}`}
                    >
                      {labels.target}: {result.targetInstance} · {labels.source}:{" "}
                      {result.backupInstance}
                    </span>
                    <span className="w-full truncate" title={result.outputPath}>
                      {labels.file}: {result.outputPath}
                    </span>
                  </div>
                  {result.conflicts.length > 0 && (
                    <ul className="space-y-1 border-t border-default px-3 py-2 text-[11px] text-amber-400">
                      {result.conflicts.map((conflict) => (
                        <li key={conflict} className="break-words">
                          {conflict}
                        </li>
                      ))}
                    </ul>
                  )}
                  <pre className="max-h-[40vh] overflow-auto border-t border-default bg-elevated p-3 font-mono text-xs text-secondary whitespace-pre-wrap break-all">
                    {result.sql}
                  </pre>
                  <p className="border-t border-default px-3 py-2 text-[11px] text-muted">
                    {labels.resultHint}
                  </p>
                </div>
              )}
            </div>
          )}
        </aside>
      </div>
    </div>
  );
}
