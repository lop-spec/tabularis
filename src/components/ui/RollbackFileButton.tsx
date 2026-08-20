import { useRef, useState } from "react";
import { FileText, Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";

interface RollbackFileButtonProps {
  rollbackFile?: string;
  onOpen: (rollbackFile: string) => Promise<void>;
}

export function RollbackFileButton({
  rollbackFile,
  onOpen,
}: RollbackFileButtonProps) {
  const { t } = useTranslation();
  const openingRef = useRef(false);
  const [isOpening, setIsOpening] = useState(false);

  if (!rollbackFile?.trim()) return null;

  const handleOpen = async () => {
    if (openingRef.current) return;
    openingRef.current = true;
    setIsOpening(true);
    try {
      await onOpen(rollbackFile);
    } finally {
      openingRef.current = false;
      setIsOpening(false);
    }
  };

  const accessibleLabel = t("editor.viewRollbackSql");

  return (
    <button
      type="button"
      onClick={() => void handleOpen()}
      disabled={isOpening}
      aria-label={accessibleLabel}
      title={`${accessibleLabel}\n${rollbackFile}`}
      className="flex items-center gap-2 px-2 @[640px]:px-3 py-1.5 bg-amber-500/10 enabled:hover:bg-amber-500/15 text-amber-300 rounded text-sm font-medium disabled:opacity-50 disabled:cursor-wait border border-amber-500/35 shrink-0 transition-colors"
    >
      {isOpening ? (
        <Loader2 size={16} className="animate-spin" />
      ) : (
        <FileText size={16} />
      )}
      <span className="hidden @[640px]:inline whitespace-nowrap">
        {t("editor.rollbackButton")}
      </span>
    </button>
  );
}
