import { useState, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { List, ChevronDown } from "lucide-react";
import type { NotebookCell } from "../../types/notebook";
import { extractOutline } from "../../utils/notebookOutline";

interface NotebookOutlineProps {
  cells: NotebookCell[];
  onScrollToCell: (cellId: string) => void;
}

function CellTypeBadge({ cellType }: { cellType: "sql" | "markdown" }) {
  if (cellType === "sql") {
    return <span className="text-green-400 mr-1">SQL</span>;
  }
  return <span className="text-blue-400 mr-1">MD</span>;
}

export function NotebookOutline({
  cells,
  onScrollToCell,
}: NotebookOutlineProps) {
  const { t } = useTranslation();
  const entries = useMemo(() => extractOutline(cells), [cells]);
  const [isCollapsed, setIsCollapsed] = useState(false);

  if (entries.length === 0) return null;

  return (
    <div className="mb-3 bg-base border border-default rounded-lg overflow-hidden">
      <button
        type="button"
        onClick={() => setIsCollapsed((v) => !v)}
        className="w-full flex items-center gap-1.5 px-3 py-1.5 bg-elevated border-b border-default hover:bg-base/40 transition-colors"
      >
        <ChevronDown
          size={12}
          className={`text-muted transition-transform ${isCollapsed ? "-rotate-90" : ""}`}
        />
        <List size={12} className="text-muted" />
        <span className="text-[10px] font-semibold uppercase text-muted flex-1 text-left">
          {t("editor.notebook.outline")}
        </span>
      </button>
      {!isCollapsed && (
        <div className="px-3 py-2 space-y-0.5">
          {entries.map((entry, i) => (
            <button
              key={`${entry.cellId}-${i}`}
              type="button"
              onClick={() => onScrollToCell(entry.cellId)}
              className="block w-full text-left text-xs text-secondary hover:text-primary transition-colors truncate"
              style={{ paddingLeft: `${(entry.level - 1) * 12}px` }}
            >
              <CellTypeBadge cellType={entry.cellType} />
              {entry.text}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
