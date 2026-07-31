import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { AlertTriangle, X } from "lucide-react";
import { Modal } from "../ui/Modal";
import type { RollbackRiskReview } from "../../utils/rollbackProtection";

interface RollbackRiskModalProps {
  isOpen: boolean;
  review: RollbackRiskReview;
  onCancel: () => void;
  onSkip: () => void;
  onExecuteUnprotected: () => void;
  onAllowImplicitCommit: () => void;
  riskDelaySeconds?: number;
}

export const RollbackRiskModal = ({
  isOpen,
  review,
  onCancel,
  onSkip,
  onExecuteUnprotected,
  onAllowImplicitCommit,
  riskDelaySeconds = 5,
}: RollbackRiskModalProps) => {
  const { t } = useTranslation();
  const [remaining, setRemaining] = useState(riskDelaySeconds);
  const [previousOpen, setPreviousOpen] = useState(isOpen);

  if (isOpen !== previousOpen) {
    setPreviousOpen(isOpen);
    setRemaining(isOpen ? riskDelaySeconds : 0);
  }

  useEffect(() => {
    if (!isOpen || remaining <= 0) return;
    const interval = setInterval(() => {
      setRemaining((current) => Math.max(0, current - 1));
    }, 1000);
    return () => clearInterval(interval);
  }, [isOpen, remaining]);

  return (
    <Modal isOpen={isOpen} onClose={onCancel}>
      <div className="bg-elevated border border-strong rounded-xl shadow-2xl w-[640px] max-w-[90vw] overflow-hidden flex flex-col">
        <div className="flex items-center justify-between p-4 border-b border-default bg-base">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-amber-900/30 rounded-lg">
              <AlertTriangle size={20} className="text-amber-400" />
            </div>
            <h2 className="text-lg font-semibold text-primary">
              {t(
                review.kind === "implicit_commit"
                  ? "editor.transactionDdlRiskTitle"
                  : "editor.rollbackRiskTitle",
              )}
            </h2>
          </div>
          <button
            onClick={onCancel}
            className="text-secondary hover:text-primary transition-colors"
          >
            <X size={20} />
          </button>
        </div>

        <div className="p-5 space-y-4">
          <p className="text-sm text-secondary leading-relaxed">
            {t(
              review.kind === "implicit_commit"
                ? "editor.transactionDdlRiskMessage"
                : "editor.rollbackRiskMessage",
              {
              count: review.statements.length,
              },
            )}
          </p>
          <div className="max-h-[300px] overflow-y-auto space-y-3 pr-1">
            {review.statements.map((statement) => (
              <div
                key={statement.index}
                className="rounded-lg border border-default bg-base/60 p-3 space-y-2"
              >
                <div className="flex items-center gap-2 text-sm font-medium text-primary">
                  <span>
                    {t("editor.rollbackRiskStatement", {
                      index: statement.index,
                    })}
                  </span>
                  {statement.destructive && (
                    <span className="rounded bg-red-900/40 px-1.5 py-0.5 text-xs text-red-300">
                      {t("editor.rollbackRiskDestructive")}
                    </span>
                  )}
                </div>
                <p className="text-xs text-amber-300">{statement.reason}</p>
                <pre className="max-h-24 overflow-auto whitespace-pre-wrap break-words rounded bg-surface-secondary p-2 text-xs text-secondary">
                  {statement.sql}
                </pre>
              </div>
            ))}
          </div>
          <p className="text-xs text-tertiary">
            {t(
              review.kind === "implicit_commit"
                ? "editor.transactionDdlRiskBoundary"
                : "editor.rollbackRiskBoundary",
            )}
          </p>
        </div>

        <div className="p-4 border-t border-default bg-base/50 flex flex-wrap justify-end gap-3">
          <button
            onClick={onCancel}
            className="px-4 py-2 text-secondary hover:text-primary transition-colors text-sm"
          >
            {t("editor.rollbackRiskCancel")}
          </button>
          {review.kind === "implicit_commit" ? (
            <button
              onClick={onAllowImplicitCommit}
              disabled={remaining > 0}
              className="px-4 py-2 bg-amber-600 hover:bg-amber-500 text-white rounded-lg text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {remaining > 0
                ? `${t("editor.transactionDdlRiskContinue")} (${remaining})`
                : t("editor.transactionDdlRiskContinue")}
            </button>
          ) : (
            <>
              <button
                onClick={onSkip}
                className="px-4 py-2 bg-surface-secondary hover:bg-surface-tertiary text-primary rounded-lg text-sm font-medium transition-colors"
              >
                {t("editor.rollbackRiskSkip")}
              </button>
              <button
                onClick={onExecuteUnprotected}
                disabled={remaining > 0}
                className="px-4 py-2 bg-red-600 hover:bg-red-500 text-white rounded-lg text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {remaining > 0
                  ? `${t("editor.rollbackRiskExecute")} (${remaining})`
                  : t("editor.rollbackRiskExecute")}
              </button>
            </>
          )}
        </div>
      </div>
    </Modal>
  );
};
