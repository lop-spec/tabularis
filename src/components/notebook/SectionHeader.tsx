import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronRight, ChevronDown, Pencil, Trash2, Check } from "lucide-react";
import type { NotebookSection } from "../../types/notebook";

interface SectionHeaderProps {
  section: NotebookSection;
  cellCount: number;
  onToggle: () => void;
  onRename: (title: string) => void;
  onDelete: () => void;
}

export function SectionHeader({
  section,
  cellCount,
  onToggle,
  onRename,
  onDelete,
}: SectionHeaderProps) {
  const { t } = useTranslation();
  const [isEditing, setIsEditing] = useState(false);
  const [editTitle, setEditTitle] = useState(section.title);

  const handleSubmit = () => {
    const trimmed = editTitle.trim();
    if (trimmed) {
      onRename(trimmed);
    }
    setIsEditing(false);
  };

  return (
    <div className="flex items-center gap-2 px-3 py-2 bg-surface-secondary rounded-lg border border-default mb-1">
      <button
        type="button"
        onClick={onToggle}
        className="text-muted hover:text-primary transition-colors"
      >
        {section.collapsed ? (
          <ChevronRight size={16} />
        ) : (
          <ChevronDown size={16} />
        )}
      </button>

      {isEditing ? (
        <div className="flex items-center gap-1 flex-1">
          <input
            type="text"
            value={editTitle}
            onChange={(e) => setEditTitle(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleSubmit();
              if (e.key === "Escape") setIsEditing(false);
            }}
            autoFocus
            className="flex-1 text-sm bg-base border border-strong rounded px-2 py-0.5 text-primary outline-none focus:border-blue-500"
          />
          <button
            type="button"
            onClick={handleSubmit}
            className="p-1 text-green-400 hover:bg-surface rounded transition-colors"
          >
            <Check size={14} />
          </button>
        </div>
      ) : (
        <span
          className="text-sm font-semibold text-primary flex-1 cursor-pointer"
          onDoubleClick={() => {
            setEditTitle(section.title);
            setIsEditing(true);
          }}
        >
          {section.title}
        </span>
      )}

      <span className="text-[10px] text-muted">
        {cellCount} {cellCount === 1 ? "cell" : "cells"}
      </span>

      <div className="flex items-center gap-0.5">
        <button
          type="button"
          onClick={() => {
            setEditTitle(section.title);
            setIsEditing(true);
          }}
          className="p-1 text-muted hover:text-secondary rounded transition-colors"
          title={t("editor.notebook.renameSection")}
        >
          <Pencil size={12} />
        </button>
        <button
          type="button"
          onClick={onDelete}
          className="p-1 text-muted hover:text-red-400 rounded transition-colors"
          title={t("editor.notebook.deleteSection")}
        >
          <Trash2 size={12} />
        </button>
      </div>
    </div>
  );
}
