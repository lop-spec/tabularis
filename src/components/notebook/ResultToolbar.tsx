import { useTranslation } from "react-i18next";
import { Download, BarChart3, X } from "lucide-react";
import type { QueryResult } from "../../types/editor";
import { resultToCsv, resultToJson } from "../../utils/notebookExport";

interface ResultToolbarProps {
  result: QueryResult;
  executionTime?: number | null;
  showChart: boolean;
  onToggleChart: () => void;
  canChart: boolean;
}

export function ResultToolbar({
  result,
  executionTime,
  showChart,
  onToggleChart,
  canChart,
}: ResultToolbarProps) {
  const { t } = useTranslation();

  const handleExportCsv = () => {
    const csv = resultToCsv(result);
    const blob = new Blob([csv], { type: "text/csv" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "result.csv";
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleExportJson = () => {
    const json = resultToJson(result);
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "result.json";
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="px-3 py-1 bg-elevated text-xs text-muted flex items-center gap-2">
      <span>
        {t("editor.notebook.cellResult", {
          count: result.rows.length,
          time: executionTime != null ? Math.round(executionTime) : "—",
        })}
      </span>
      <div className="flex-1" />
      <div className="flex items-center gap-0.5">
        {canChart && (
          <button
            type="button"
            onClick={onToggleChart}
            className={`p-1 rounded transition-colors ${
              showChart
                ? "text-blue-400 bg-blue-500/15"
                : "text-muted hover:text-secondary hover:bg-surface-secondary"
            }`}
            title={t("editor.notebook.toggleChart")}
          >
            {showChart ? <X size={12} /> : <BarChart3 size={12} />}
          </button>
        )}
        <button
          type="button"
          onClick={handleExportCsv}
          className="p-1 text-muted hover:text-secondary hover:bg-surface-secondary rounded transition-colors"
          title={t("editor.notebook.exportCsv")}
        >
          <span className="flex items-center gap-0.5">
            <Download size={12} />
            <span className="text-[9px]">CSV</span>
          </span>
        </button>
        <button
          type="button"
          onClick={handleExportJson}
          className="p-1 text-muted hover:text-secondary hover:bg-surface-secondary rounded transition-colors"
          title={t("editor.notebook.exportJson")}
        >
          <span className="flex items-center gap-0.5">
            <Download size={12} />
            <span className="text-[9px]">JSON</span>
          </span>
        </button>
      </div>
    </div>
  );
}
