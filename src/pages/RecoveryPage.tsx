import { invoke } from "@tauri-apps/api/core";
import {
  AlertTriangle,
  CheckCircle2,
  Clock3,
  Copy,
  Database,
  FileText,
  Loader2,
  RefreshCw,
  ShieldCheck,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useDatabase } from "../hooks/useDatabase";

interface RecoveryStatementSummary {
  id: string;
  index: number;
  executedAt: string;
  sql: string;
  category: "ddl" | "dml";
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
  "w-full rounded-lg border border-default bg-base px-3 py-2 text-sm text-primary outline-none transition-colors placeholder:text-muted focus:border-blue-500/60";

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

function displayDatabase(value: string | string[]): string {
  return Array.isArray(value) ? value.join(", ") : value;
}

export function RecoveryPage() {
  const { i18n } = useTranslation();
  const zh = i18n.language.toLowerCase().startsWith("zh");
  const { activeConnectionId, connections } = useDatabase();
  const labels = zh
    ? {
        title: "数据恢复",
        subtitle: "按执行记录选择 DDL / DML，只读对比备份实例后生成最小行级恢复 SQL。",
        activeOnly: "仅当前连接",
        allConnections: "全部连接",
        from: "开始时间",
        to: "结束时间",
        refresh: "刷新",
        selectFiltered: "选择筛选结果",
        clear: "清空选择",
        noRuns: "这个时间范围内没有可恢复记录",
        noRunsHint: "记录来自已开启回滚保护的 MySQL / MariaDB Run All。",
        runAll: "Run All",
        rows: "行",
        fields: "影响字段",
        condition: "条件",
        exact: "精确记录",
        inexact: "不可精确恢复",
        backup: "备份实例",
        backupHint: "直接复用已保存连接的数据库、SSH 与凭据配置。",
        connection: "已保存连接",
        pickConnection: "请选择 MySQL / MariaDB 连接",
        noConnections: "暂无可用的 MySQL / MariaDB 连接，请先到“连接”中添加。",
        test: "测试连接",
        tested: "连接成功",
        generate: "只读对比并生成 SQL",
        selected: "已选择",
        statements: "条语句",
        output: "恢复 SQL",
        outputHint: "只生成文件，不会自动执行。",
        copy: "复制 SQL",
        copied: "已复制",
        target: "目标",
        backupInstance: "备份",
        steps: "生成步骤",
        unchanged: "无需恢复",
        conflicts: "冲突",
        pickFirst: "请先选择 Run All 或单条 DDL / DML。",
        pickBackup: "请先选择备份连接。",
        oneConnection: "一次只能恢复同一个目标连接的记录。",
      }
    : {
        title: "Data recovery",
        subtitle:
          "Select executed DDL/DML, compare a backup instance read-only, then generate minimal row-level recovery SQL.",
        activeOnly: "Active connection only",
        allConnections: "All connections",
        from: "From",
        to: "To",
        refresh: "Refresh",
        selectFiltered: "Select filtered",
        clear: "Clear selection",
        noRuns: "No recovery records in this time range",
        noRunsHint: "Records come from protected MySQL / MariaDB Run All executions.",
        runAll: "Run All",
        rows: "rows",
        fields: "Fields",
        condition: "Condition",
        exact: "Exact",
        inexact: "Not exactly recoverable",
        backup: "Backup instance",
        backupHint: "Reuse the database, SSH, and credentials from a saved connection.",
        connection: "Saved connection",
        pickConnection: "Select a MySQL / MariaDB connection",
        noConnections: "No saved MySQL / MariaDB connections. Add one in Connections first.",
        test: "Test connection",
        tested: "Connected",
        generate: "Compare read-only and generate SQL",
        selected: "Selected",
        statements: "statements",
        output: "Recovery SQL",
        outputHint: "The file is generated only; it is never executed automatically.",
        copy: "Copy SQL",
        copied: "Copied",
        target: "Target",
        backupInstance: "Backup",
        steps: "Generated steps",
        unchanged: "No change",
        conflicts: "Conflicts",
        pickFirst: "Select a Run All or individual DDL/DML first.",
        pickBackup: "Select a backup connection first.",
        oneConnection: "A recovery selection can only target one connection.",
      };

  const now = useMemo(() => new Date(), []);
  const [startedAfter, setStartedAfter] = useState(() =>
    localDateTimeInput(new Date(now.getTime() - 24 * 60 * 60 * 1_000)),
  );
  const [startedBefore, setStartedBefore] = useState(() => localDateTimeInput(now));
  const [activeOnly, setActiveOnly] = useState(true);
  const [runs, setRuns] = useState<RecoveryRunSummary[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [testState, setTestState] = useState<"idle" | "testing" | "ok">("idle");
  const [generating, setGenerating] = useState(false);
  const [copied, setCopied] = useState(false);
  const [result, setResult] = useState<RecoveryCompareResponse | null>(null);
  const [backupConnectionId, setBackupConnectionId] = useState("");

  const backupConnections = useMemo(
    () =>
      connections.filter((connection) =>
        ["mysql", "mariadb"].includes(connection.params.driver.toLowerCase()),
      ),
    [connections],
  );
  const selectedBackupConnection = useMemo(
    () => backupConnections.find((connection) => connection.id === backupConnectionId),
    [backupConnectionId, backupConnections],
  );

  const loadRuns = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await invoke<RecoveryRunSummary[]>("list_recovery_runs", {
        connectionId: activeOnly ? activeConnectionId : null,
        startedAfter: startedAfter ? new Date(startedAfter).toISOString() : null,
        startedBefore: startedBefore ? new Date(startedBefore).toISOString() : null,
      });
      setRuns(response);
      const available = new Set(
        response.flatMap((run) => run.statements.map((statement) => statement.id)),
      );
      setSelected((current) => new Set([...current].filter((id) => available.has(id))));
    } catch (loadError) {
      setError(errorText(loadError));
    } finally {
      setLoading(false);
    }
  }, [activeConnectionId, activeOnly, startedAfter, startedBefore]);

  useEffect(() => {
    void loadRuns();
  }, [loadRuns]);

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

  const toggleStatement = (run: RecoveryRunSummary, statementId: string) => {
    setError(null);
    setResult(null);
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(statementId)) {
        next.delete(statementId);
        return next;
      }
      const otherConnectionSelected = runs.some(
        (candidate) =>
          candidate.connectionId !== run.connectionId &&
          candidate.statements.some((statement) => next.has(statement.id)),
      );
      if (otherConnectionSelected) {
        setError(labels.oneConnection);
        return current;
      }
      next.add(statementId);
      return next;
    });
  };

  const toggleRun = (run: RecoveryRunSummary) => {
    setError(null);
    setResult(null);
    setSelected((current) => {
      const next = new Set(current);
      const allSelected =
        run.statements.length > 0 &&
        run.statements.every((statement) => next.has(statement.id));
      if (allSelected) {
        run.statements.forEach((statement) => next.delete(statement.id));
        return next;
      }
      const otherConnectionSelected = runs.some(
        (candidate) =>
          candidate.connectionId !== run.connectionId &&
          candidate.statements.some((statement) => next.has(statement.id)),
      );
      if (otherConnectionSelected) {
        setError(labels.oneConnection);
        return current;
      }
      run.statements.forEach((statement) => next.add(statement.id));
      return next;
    });
  };

  const selectFiltered = () => {
    if (runs.length === 0) return;
    const firstConnection = runs[0].connectionId;
    setSelected(
      new Set(
        runs
          .filter((run) => run.connectionId === firstConnection)
          .flatMap((run) => run.statements.map((statement) => statement.id)),
      ),
    );
    if (runs.some((run) => run.connectionId !== firstConnection)) {
      setError(labels.oneConnection);
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
      await invoke<string>("test_saved_connection", {
        connectionId: backupConnectionId,
      });
      setTestState("ok");
    } catch (testError) {
      setTestState("idle");
      setError(errorText(testError));
    }
  };

  const generate = async () => {
    if (!targetConnectionId || selected.size === 0) {
      setError(labels.pickFirst);
      return;
    }
    if (selectedConnectionIds.size !== 1) {
      setError(labels.oneConnection);
      return;
    }
    if (!backupConnectionId) {
      setError(labels.pickBackup);
      return;
    }
    setGenerating(true);
    setError(null);
    setResult(null);
    try {
      const response = await invoke<RecoveryCompareResponse>("generate_recovery_sql", {
        connectionId: targetConnectionId,
        selection: {
          runIds: selectedRuns.map((run) => run.runId),
          statementIds: [...selected],
        },
        backupConnectionId,
      });
      setResult(response);
    } catch (generateError) {
      setError(errorText(generateError));
    } finally {
      setGenerating(false);
    }
  };

  const selectBackupConnection = (connectionId: string) => {
    setBackupConnectionId(connectionId);
    setTestState("idle");
    setResult(null);
  };

  return (
    <div className="flex h-full min-h-0 flex-col bg-base">
      <header className="border-b border-default px-6 py-4">
        <div className="flex items-start justify-between gap-4">
          <div>
            <div className="flex items-center gap-2">
              <ShieldCheck className="text-blue-400" size={20} />
              <h1 className="text-lg font-semibold text-primary">{labels.title}</h1>
            </div>
            <p className="mt-1 text-sm text-muted">{labels.subtitle}</p>
          </div>
          <div className="rounded-lg border border-blue-500/20 bg-blue-500/5 px-3 py-2 text-xs text-secondary">
            {labels.outputHint}
          </div>
        </div>
      </header>

      <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1.15fr)_minmax(360px,0.85fr)]">
        <section className="flex min-h-0 flex-col border-r border-default">
          <div className="space-y-3 border-b border-default bg-elevated px-5 py-4">
            <div className="grid grid-cols-2 gap-3">
              <label className="space-y-1">
                <span className="text-[11px] font-medium uppercase tracking-wide text-muted">
                  {labels.from}
                </span>
                <input
                  className={inputClass}
                  type="datetime-local"
                  value={startedAfter}
                  onChange={(event) => setStartedAfter(event.target.value)}
                />
              </label>
              <label className="space-y-1">
                <span className="text-[11px] font-medium uppercase tracking-wide text-muted">
                  {labels.to}
                </span>
                <input
                  className={inputClass}
                  type="datetime-local"
                  value={startedBefore}
                  onChange={(event) => setStartedBefore(event.target.value)}
                />
              </label>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <button
                type="button"
                onClick={() => setActiveOnly((value) => !value)}
                className={`rounded-lg border px-3 py-1.5 text-xs transition-colors ${
                  activeOnly
                    ? "border-blue-500/40 bg-blue-500/10 text-blue-300"
                    : "border-default text-secondary hover:bg-surface-secondary"
                }`}
              >
                {activeOnly ? labels.activeOnly : labels.allConnections}
              </button>
              <button
                type="button"
                onClick={() => void loadRuns()}
                className="inline-flex items-center gap-1.5 rounded-lg border border-default px-3 py-1.5 text-xs text-secondary hover:bg-surface-secondary"
              >
                <RefreshCw className={loading ? "animate-spin" : ""} size={13} />
                {labels.refresh}
              </button>
              <button
                type="button"
                onClick={selectFiltered}
                className="rounded-lg border border-default px-3 py-1.5 text-xs text-secondary hover:bg-surface-secondary"
              >
                {labels.selectFiltered}
              </button>
              <button
                type="button"
                onClick={() => {
                  setSelected(new Set());
                  setResult(null);
                }}
                className="rounded-lg px-3 py-1.5 text-xs text-muted hover:bg-surface-secondary hover:text-secondary"
              >
                {labels.clear}
              </button>
              <span className="ml-auto text-xs text-muted">
                {labels.selected} {selected.size} {labels.statements}
              </span>
            </div>
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto p-4">
            {!loading && runs.length === 0 ? (
              <div className="flex h-full min-h-56 flex-col items-center justify-center text-center">
                <Clock3 className="mb-3 text-muted" size={28} />
                <p className="text-sm text-secondary">{labels.noRuns}</p>
                <p className="mt-1 max-w-sm text-xs text-muted">{labels.noRunsHint}</p>
              </div>
            ) : (
              <div className="space-y-3">
                {runs.map((run) => {
                  const runSelected =
                    run.statements.length > 0 &&
                    run.statements.every((statement) => selected.has(statement.id));
                  const selectedCount = run.statements.filter((statement) =>
                    selected.has(statement.id),
                  ).length;
                  return (
                    <article
                      key={run.runId}
                      className="overflow-hidden rounded-xl border border-default bg-elevated"
                    >
                      <button
                        type="button"
                        onClick={() => toggleRun(run)}
                        className="flex w-full items-start gap-3 px-4 py-3 text-left hover:bg-surface-secondary/60"
                      >
                        <input
                          type="checkbox"
                          checked={runSelected}
                          readOnly
                          className="mt-1 h-4 w-4 accent-blue-500"
                        />
                        <div className="min-w-0 flex-1">
                          <div className="flex flex-wrap items-center gap-2">
                            <span className="rounded bg-blue-500/10 px-1.5 py-0.5 font-mono text-[11px] text-blue-300">
                              {run.shortId}
                            </span>
                            <span className="text-sm font-medium text-primary">
                              {labels.runAll}
                            </span>
                            <span className="truncate text-xs text-muted">
                              {run.connectionName} · {run.database}
                            </span>
                          </div>
                          <div className="mt-1 flex gap-3 text-[11px] text-muted">
                            <span>{displayTime(run.startedAt)}</span>
                            <span>
                              {selectedCount}/{run.statementCount}
                            </span>
                          </div>
                        </div>
                      </button>

                      <div className="border-t border-default">
                        {run.statements.map((statement) => (
                          <button
                            type="button"
                            key={statement.id}
                            onClick={() => toggleStatement(run, statement.id)}
                            className="flex w-full gap-3 border-b border-default/60 px-4 py-3 text-left last:border-b-0 hover:bg-surface-secondary/50"
                          >
                            <input
                              type="checkbox"
                              checked={selected.has(statement.id)}
                              readOnly
                              className="mt-1 h-4 w-4 accent-blue-500"
                            />
                            <div className="min-w-0 flex-1">
                              <div className="flex flex-wrap items-center gap-2 text-[11px]">
                                <span className="font-mono text-blue-300">{statement.id}</span>
                                <span className="rounded bg-surface-secondary px-1.5 py-0.5 uppercase text-secondary">
                                  {statement.operation}
                                </span>
                                <span className="text-muted">
                                  {statement.schema}.{statement.table}
                                </span>
                                <span
                                  className={
                                    statement.exact ? "text-emerald-400" : "text-amber-400"
                                  }
                                >
                                  {statement.exact ? labels.exact : labels.inexact}
                                </span>
                              </div>
                              <pre className="mt-1.5 overflow-hidden text-ellipsis whitespace-nowrap font-mono text-xs text-secondary">
                                {statement.sql}
                              </pre>
                              <div className="mt-1.5 flex flex-wrap gap-x-3 gap-y-1 text-[11px] text-muted">
                                <span>
                                  {labels.rows}: {statement.rowCount}
                                </span>
                                {statement.affectedColumns.length > 0 && (
                                  <span>
                                    {labels.fields}: {statement.affectedColumns.join(", ")}
                                  </span>
                                )}
                                {statement.condition && (
                                  <span className="max-w-full truncate">
                                    {labels.condition}: {statement.condition}
                                  </span>
                                )}
                              </div>
                            </div>
                          </button>
                        ))}
                      </div>
                    </article>
                  );
                })}
              </div>
            )}
          </div>
        </section>

        <aside className="min-h-0 overflow-y-auto p-5">
          <div className="rounded-xl border border-default bg-elevated p-4">
            <div className="flex items-center gap-2">
              <Database size={17} className="text-blue-400" />
              <h2 className="text-sm font-semibold text-primary">{labels.backup}</h2>
            </div>
            <p className="mt-1 text-xs text-muted">{labels.backupHint}</p>

            <label className="mt-4 block space-y-1">
              <span className="text-[11px] text-muted">{labels.connection}</span>
              <select
                className={inputClass}
                value={backupConnectionId}
                onChange={(event) => selectBackupConnection(event.target.value)}
              >
                <option value="">
                  {backupConnections.length > 0 ? labels.pickConnection : labels.noConnections}
                </option>
                {backupConnections.map((connection) => (
                  <option key={connection.id} value={connection.id}>
                    {connection.name} · {connection.params.driver.toUpperCase()}
                    {connection.params.host
                      ? ` · ${connection.params.host}${
                          connection.params.port ? `:${connection.params.port}` : ""
                        }`
                      : ""}
                    {` · ${displayDatabase(connection.params.database)}`}
                  </option>
                ))}
              </select>
            </label>
            {selectedBackupConnection && (
              <div className="mt-3 rounded-lg border border-default/70 bg-base/60 px-3 py-2 text-xs text-muted">
                <span className="font-medium text-secondary">{selectedBackupConnection.name}</span>
                <span className="ml-2 uppercase">
                  {selectedBackupConnection.params.driver}
                </span>
                {selectedBackupConnection.params.ssh_enabled && (
                  <span className="ml-2 text-blue-400">SSH</span>
                )}
              </div>
            )}
            <button
              type="button"
              disabled={testState === "testing" || !backupConnectionId}
              onClick={() => void testBackup()}
              className="mt-4 inline-flex w-full items-center justify-center gap-2 rounded-lg border border-default px-3 py-2 text-sm text-secondary transition-colors hover:bg-surface-secondary disabled:cursor-not-allowed disabled:opacity-50"
            >
              {testState === "testing" ? (
                <Loader2 className="animate-spin" size={15} />
              ) : testState === "ok" ? (
                <CheckCircle2 className="text-emerald-400" size={15} />
              ) : (
                <Database size={15} />
              )}
              {testState === "ok" ? labels.tested : labels.test}
            </button>
          </div>

          {error && (
            <div className="mt-4 flex gap-2 rounded-xl border border-red-500/25 bg-red-500/5 p-3 text-xs text-red-300">
              <AlertTriangle className="mt-0.5 shrink-0" size={15} />
              <span className="break-words">{error}</span>
            </div>
          )}

          <button
            type="button"
            disabled={
              generating ||
              selected.size === 0 ||
              selectedConnectionIds.size !== 1 ||
              !backupConnectionId
            }
            onClick={() => void generate()}
            className="mt-4 inline-flex w-full items-center justify-center gap-2 rounded-lg bg-blue-600 px-4 py-2.5 text-sm font-medium text-white transition-colors hover:bg-blue-500 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {generating ? (
              <Loader2 className="animate-spin" size={16} />
            ) : (
              <ShieldCheck size={16} />
            )}
            {labels.generate}
          </button>

          {result && (
            <div className="mt-4 overflow-hidden rounded-xl border border-default bg-elevated">
              <div className="border-b border-default p-4">
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-2">
                    <FileText size={16} className="text-blue-400" />
                    <h2 className="text-sm font-semibold text-primary">{labels.output}</h2>
                  </div>
                  <button
                    type="button"
                    onClick={() => {
                      void navigator.clipboard.writeText(result.sql);
                      setCopied(true);
                      window.setTimeout(() => setCopied(false), 1_500);
                    }}
                    className="inline-flex items-center gap-1.5 rounded-lg border border-default px-2.5 py-1.5 text-xs text-secondary hover:bg-surface-secondary"
                  >
                    <Copy size={12} />
                    {copied ? labels.copied : labels.copy}
                  </button>
                </div>
                <div className="mt-3 grid grid-cols-3 gap-2">
                  <div className="rounded-lg bg-surface-secondary p-2">
                    <div className="text-[10px] uppercase text-muted">{labels.steps}</div>
                    <div className="mt-0.5 text-sm font-medium text-primary">
                      {result.generatedSteps}
                    </div>
                  </div>
                  <div className="rounded-lg bg-surface-secondary p-2">
                    <div className="text-[10px] uppercase text-muted">{labels.unchanged}</div>
                    <div className="mt-0.5 text-sm font-medium text-primary">
                      {result.unchangedRows}
                    </div>
                  </div>
                  <div className="rounded-lg bg-surface-secondary p-2">
                    <div className="text-[10px] uppercase text-muted">{labels.conflicts}</div>
                    <div
                      className={`mt-0.5 text-sm font-medium ${
                        result.conflicts.length ? "text-amber-400" : "text-emerald-400"
                      }`}
                    >
                      {result.conflicts.length}
                    </div>
                  </div>
                </div>
                <div className="mt-3 space-y-1 text-[11px] text-muted">
                  <div className="truncate">
                    {labels.target}: {result.targetInstance}
                  </div>
                  <div className="truncate">
                    {labels.backupInstance}: {result.backupInstance}
                  </div>
                  <div className="break-all font-mono text-secondary">{result.outputPath}</div>
                </div>
                {result.conflicts.length > 0 && (
                  <ul className="mt-3 space-y-1 rounded-lg border border-amber-500/20 bg-amber-500/5 p-2 text-[11px] text-amber-300">
                    {result.conflicts.map((conflict) => (
                      <li key={conflict}>{conflict}</li>
                    ))}
                  </ul>
                )}
              </div>
              <pre className="max-h-80 overflow-auto whitespace-pre-wrap break-words p-4 font-mono text-[11px] leading-5 text-secondary">
                {result.sql}
              </pre>
            </div>
          )}
        </aside>
      </div>
    </div>
  );
}
