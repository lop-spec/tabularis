import { useState } from "react";
import { Check, ChevronDown, ChevronUp, Copy } from "lucide-react";
import type { TFunction } from "i18next";
import { copyTextToClipboard } from "../../utils/clipboard";

interface ErrorDisplayProps {
  error: string;
  t: TFunction;
}

export function ErrorDisplay({ error, t }: ErrorDisplayProps) {
  const [showDetails, setShowDetails] = useState(false);
  const [copied, setCopied] = useState(false);

  const separatorIndex = error.indexOf("\n\n");
  const hasDetails = separatorIndex !== -1 && separatorIndex < error.length - 2;
  const brief = hasDetails ? error.slice(0, separatorIndex) : error;
  const details = hasDetails ? error.slice(separatorIndex + 2) : "";

  const copyError = async () => {
    try {
      await copyTextToClipboard(`Error: ${error}`);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      // copyTextToClipboard already logs the browser clipboard error.
    }
  };

  return (
    <div className="relative p-4 pr-12 text-red-400 font-mono text-sm bg-red-900/10 h-full overflow-auto select-text">
      <button
        type="button"
        onClick={copyError}
        aria-label={t("sidebar.copyError")}
        title={t("sidebar.copyError")}
        className="absolute top-2 right-2 p-1.5 rounded text-red-300/70 hover:text-red-300 hover:bg-red-400/10 transition-colors cursor-pointer select-none"
      >
        {copied ? (
          <Check size={14} className="text-green-500" />
        ) : (
          <Copy size={14} />
        )}
      </button>
      <div className="whitespace-pre-wrap">Error: {brief}</div>
      {hasDetails && (
        <>
          <button
            type="button"
            onClick={() => setShowDetails((v) => !v)}
            className="mt-2 flex items-center gap-1 text-xs text-red-300/70 hover:text-red-300 transition-colors cursor-pointer"
          >
            {showDetails ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
            {showDetails
              ? t("editor.hideErrorDetails")
              : t("editor.showErrorDetails")}
          </button>
          {showDetails && (
            <div className="mt-2 whitespace-pre-wrap text-red-400/60 border-t border-red-400/20 pt-2">
              {details}
            </div>
          )}
        </>
      )}
    </div>
  );
}
