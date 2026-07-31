import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import {
  Activity,
  AlertCircle,
  ChevronDown,
  ChevronRight,
  Cpu,
  HardDrive,
  Layers,
  Loader2,
  MemoryStick,
  RefreshCw,
} from "lucide-react";
import clsx from "clsx";
import { useTaskManager } from "../hooks/useTaskManager";
import {
  formatBytes,
  formatCpuPercent,
  formatMemoryBar,
} from "../utils/taskManager";
import type {
  TabularisChildProcess,
  TabularisSelfStats,
} from "../utils/taskManager";

interface StatCardProps {
  icon: React.ReactNode;
  label: string;
  value: string;
  suffix?: string;
}

const StatCard = ({ icon, label, value, suffix }: StatCardProps) => (
  <div className="bg-base rounded-lg p-3 border border-default">
    <div className="flex items-center gap-2 mb-2">
      {icon}
      <span className="text-xs text-muted font-medium uppercase tracking-wide">
        {label}
      </span>
    </div>
    <p className="text-lg font-bold text-primary">
      {value}
      {suffix && (
        <span className="text-xs font-normal text-muted ml-1">{suffix}</span>
      )}
    </p>
  </div>
);

const TabularisSelfPanel = ({ stats }: { stats: TabularisSelfStats }) => {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const [children, setChildren] = useState<TabularisChildProcess[]>([]);
  const [loadingChildren, setLoadingChildren] = useState(false);

  const loadChildren = useCallback(async () => {
    setLoadingChildren(true);
    try {
      setChildren(
        await invoke<TabularisChildProcess[]>("get_tabularis_children"),
      );
    } finally {
      setLoadingChildren(false);
    }
  }, []);

  const toggleChildren = useCallback(() => {
    if (stats.child_count === 0) return;
    if (!expanded) void loadChildren();
    setExpanded((current) => !current);
  }, [expanded, loadChildren, stats.child_count]);

  return (
    <section className="bg-elevated border border-default rounded-xl p-5">
      <h2 className="text-sm font-semibold text-primary mb-4 flex items-center gap-2">
        <Activity size={15} className="text-blue-400" />
        {t("taskManager.tabularisProcess.title")}
        <span className="ml-auto text-xs text-muted font-mono">
          PID {stats.pid}
        </span>
      </h2>
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        <StatCard
          icon={<Cpu size={14} className="text-blue-400" />}
          label={t("taskManager.tabularisProcess.cpu")}
          value={formatCpuPercent(stats.cpu_percent)}
        />
        <StatCard
          icon={<MemoryStick size={14} className="text-purple-400" />}
          label={t("taskManager.tabularisProcess.ram")}
          value={formatBytes(stats.self_memory_bytes)}
        />
        <StatCard
          icon={<HardDrive size={14} className="text-green-400" />}
          label={t("taskManager.tabularisProcess.diskRead")}
          value={formatBytes(stats.disk_read_bytes)}
          suffix="/s"
        />
        <StatCard
          icon={<HardDrive size={14} className="text-orange-400" />}
          label={t("taskManager.tabularisProcess.diskWrite")}
          value={formatBytes(stats.disk_write_bytes)}
          suffix="/s"
        />
      </div>
      <button
        type="button"
        onClick={toggleChildren}
        className={clsx(
          "mt-3 w-full flex items-center gap-1.5 text-xs text-muted",
          stats.child_count > 0 &&
            "hover:text-primary transition-colors cursor-pointer",
        )}
      >
        {stats.child_count > 0 ? (
          expanded ? (
            <ChevronDown size={12} />
          ) : (
            <ChevronRight size={12} />
          )
        ) : (
          <Layers size={12} />
        )}
        {stats.child_count > 0
          ? t("taskManager.tabularisProcess.childCount", {
              count: stats.child_count,
            })
          : t("taskManager.tabularisProcess.noChildren")}
        <span className="ml-auto opacity-60">
          {t("taskManager.tabularisProcess.treeTotal", {
            size: formatBytes(stats.total_memory_bytes),
          })}
        </span>
      </button>
      {expanded && (
        <div className="mt-3 rounded-lg border border-default overflow-hidden">
          {loadingChildren ? (
            <div className="flex items-center justify-center gap-2 py-6 text-muted text-xs">
              <Loader2 size={14} className="animate-spin" />
              {t("taskManager.tabularisProcess.loadingProcesses")}
            </div>
          ) : (
            <table className="w-full text-xs">
              <thead>
                <tr className="border-b border-default bg-base/50">
                  <th className="px-3 py-2 text-left text-muted">PID</th>
                  <th className="px-3 py-2 text-left text-muted">
                    {t("taskManager.tabularisProcess.colName")}
                  </th>
                  <th className="px-3 py-2 text-left text-muted">CPU</th>
                  <th className="px-3 py-2 text-left text-muted">RAM</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-default">
                {children.map((child) => (
                  <tr key={child.pid}>
                    <td className="px-3 py-2 font-mono text-muted">
                      {child.pid}
                    </td>
                    <td className="px-3 py-2 text-secondary">{child.name}</td>
                    <td className="px-3 py-2 text-secondary">
                      {formatCpuPercent(child.cpu_percent)}
                    </td>
                    <td className="px-3 py-2 text-secondary">
                      {formatBytes(child.memory_bytes)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}
    </section>
  );
};

export const TaskManagerPage = () => {
  const { t } = useTranslation();
  const { systemStats, loading, error, refresh } = useTaskManager();
  const memoryPercent = systemStats
    ? formatMemoryBar(systemStats.memory_used, systemStats.memory_total)
    : 0;

  return (
    <div className="h-screen flex flex-col bg-base text-primary overflow-hidden">
      <header className="flex items-center justify-between px-6 py-4 border-b border-default bg-elevated shrink-0">
        <div className="flex items-center gap-3">
          <div className="p-2 rounded-lg bg-blue-500/20 border border-blue-500/30">
            <Activity size={18} className="text-blue-400" />
          </div>
          <div>
            <h1 className="text-base font-semibold text-primary">
              {t("taskManager.header.title")}
            </h1>
            <p className="text-xs text-muted">
              {t("taskManager.header.subtitle")}
            </p>
          </div>
        </div>
        <button
          type="button"
          onClick={() => void refresh()}
          disabled={loading}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm text-muted hover:text-primary hover:bg-surface-secondary/50 disabled:opacity-50"
        >
          <RefreshCw size={14} className={clsx(loading && "animate-spin")} />
          {t("taskManager.header.refresh")}
        </button>
      </header>

      <main className="flex-1 overflow-y-auto p-6 space-y-5">
        {error && (
          <div className="flex items-center gap-3 p-3 rounded-lg bg-red-500/10 border border-red-500/20 text-red-400 text-sm">
            <AlertCircle size={16} />
            <span>{error}</span>
          </div>
        )}
        <section className="bg-elevated border border-default rounded-xl p-5">
          <h2 className="text-sm font-semibold text-primary mb-4 flex items-center gap-2">
            <Cpu size={15} className="text-blue-400" />
            {t("taskManager.systemResources.title")}
          </h2>
          <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
            <StatCard
              icon={<Cpu size={14} className="text-blue-400" />}
              label={t("taskManager.systemResources.cpu")}
              value={
                systemStats
                  ? formatCpuPercent(systemStats.cpu_percent)
                  : "—"
              }
            />
            <StatCard
              icon={<MemoryStick size={14} className="text-purple-400" />}
              label={`${t("taskManager.systemResources.memory")} ${memoryPercent}%`}
              value={
                systemStats
                  ? `${formatBytes(systemStats.memory_used)} / ${formatBytes(systemStats.memory_total)}`
                  : "—"
              }
            />
            <StatCard
              icon={<HardDrive size={14} className="text-green-400" />}
              label={t("taskManager.systemResources.diskRead")}
              value={
                systemStats ? formatBytes(systemStats.disk_read_bytes) : "—"
              }
              suffix="/s"
            />
            <StatCard
              icon={<Layers size={14} className="text-orange-400" />}
              label={t("taskManager.systemResources.processes")}
              value={systemStats ? String(systemStats.process_count) : "—"}
            />
          </div>
        </section>
        {systemStats?.tabularis && (
          <TabularisSelfPanel stats={systemStats.tabularis} />
        )}
      </main>
    </div>
  );
};
