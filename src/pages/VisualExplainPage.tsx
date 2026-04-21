import { useCallback, useEffect, useState } from "react";
import { useLocation } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { FileJson, FolderOpen, Loader2, RefreshCw } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { ExplainPlan } from "../types/explain";
import { VisualExplainView } from "../components/explain/VisualExplainView";
import type { ExplainViewMode } from "../components/modals/visual-explain/ExplainSummaryBar";
import {
  getExplainFileName,
  parseExplainFileParam,
} from "../utils/explainImport";
import { useSettings } from "../hooks/useSettings";

export const VisualExplainPage = () => {
  const { t } = useTranslation();
  const { settings } = useSettings();
  const { search } = useLocation();
  const initialParamPath = parseExplainFileParam(search);

  const [filePath, setFilePath] = useState<string | null>(initialParamPath);
  const [plan, setPlan] = useState<ExplainPlan | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [viewMode, setViewMode] = useState<ExplainViewMode>("graph");
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);

  const loadPlan = useCallback(async (path: string) => {
    setIsLoading(true);
    setError(null);
    setPlan(null);
    setSelectedNodeId(null);
    try {
      const result = await invoke<ExplainPlan>("load_explain_from_file", {
        path,
      });
      setPlan(result);
      setSelectedNodeId(result.root.id);
    } catch (err) {
      setError(String(err));
    } finally {
      setIsLoading(false);
    }
  }, []);

  // On mount: if launched with ?file=... (or from CLI via pending slot), load it.
  useEffect(() => {
    let cancelled = false;

    const bootstrap = async () => {
      if (initialParamPath) {
        await loadPlan(initialParamPath);
        return;
      }
      try {
        const pending = await invoke<string | null>("get_pending_explain_file");
        if (!cancelled && pending) {
          setFilePath(pending);
          await loadPlan(pending);
        }
      } catch (err) {
        if (!cancelled) setError(String(err));
      }
    };

    bootstrap();
    return () => {
      cancelled = true;
    };
  }, [initialParamPath, loadPlan]);

  const handlePickFile = useCallback(async () => {
    const selected = await openDialog({
      multiple: false,
      filters: [
        { name: "Explain", extensions: ["json", "txt"] },
        { name: "All files", extensions: ["*"] },
      ],
    });
    if (typeof selected === "string" && selected.trim().length > 0) {
      setFilePath(selected);
      await loadPlan(selected);
    }
  }, [loadPlan]);

  const handleReload = useCallback(() => {
    if (filePath) loadPlan(filePath);
  }, [filePath, loadPlan]);

  const fileLabel = filePath ? getExplainFileName(filePath) : null;

  return (
    <div className="w-screen h-screen flex flex-col bg-base">
      <div className="flex items-center justify-between px-4 py-3 border-b border-default bg-elevated shrink-0">
        <div className="flex items-center gap-3 min-w-0">
          <div className="p-2 bg-green-900/30 rounded-lg">
            <FileJson size={18} className="text-green-400" />
          </div>
          <div className="min-w-0">
            <h1 className="text-base font-semibold text-primary truncate">
              {t("visualExplainPage.title")}
            </h1>
            {fileLabel ? (
              <p
                className="text-xs text-muted font-mono truncate"
                title={filePath ?? undefined}
              >
                {fileLabel}
              </p>
            ) : (
              <p className="text-xs text-muted">
                {t("visualExplainPage.noFile")}
              </p>
            )}
          </div>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={handlePickFile}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm text-muted hover:text-primary hover:bg-surface-secondary/50 transition-colors"
          >
            <FolderOpen size={14} />
            {t("visualExplainPage.openFile")}
          </button>
          <button
            onClick={handleReload}
            disabled={!filePath || isLoading}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm text-muted hover:text-primary hover:bg-surface-secondary/50 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            <RefreshCw
              size={14}
              className={isLoading ? "animate-spin" : ""}
            />
            {t("visualExplainPage.reload")}
          </button>
        </div>
      </div>

      <div className="flex-1 min-h-0">
        {!filePath && !isLoading ? (
          <div className="flex flex-col items-center justify-center h-full gap-3 text-muted">
            <FileJson size={32} className="opacity-30" />
            <p className="text-sm">{t("visualExplainPage.emptyHint")}</p>
            <button
              onClick={handlePickFile}
              className="flex items-center gap-1.5 px-4 py-2 bg-green-600 hover:bg-green-500 text-white rounded-lg text-sm font-medium transition-colors"
            >
              <FolderOpen size={14} />
              {t("visualExplainPage.openFile")}
            </button>
          </div>
        ) : isLoading && !plan ? (
          <div className="flex flex-col items-center justify-center h-full gap-2 text-muted">
            <Loader2 size={24} className="animate-spin" />
            <span className="text-sm">{t("visualExplainPage.loading")}</span>
          </div>
        ) : (
          <VisualExplainView
            plan={plan}
            isLoading={isLoading}
            error={error}
            viewMode={viewMode}
            onViewModeChange={setViewMode}
            selectedNodeId={selectedNodeId}
            onSelectNode={setSelectedNodeId}
            aiEnabled={!!settings.aiEnabled}
          />
        )}
      </div>
    </div>
  );
};
