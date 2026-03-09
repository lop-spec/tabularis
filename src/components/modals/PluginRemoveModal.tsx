import { useTranslation } from "react-i18next";
import { Trash2, X } from "lucide-react";
import { Modal } from "../ui/Modal";

interface PluginRemoveModalProps {
  isOpen: boolean;
  onClose: () => void;
  pluginName: string;
  onConfirm: () => void;
}

export const PluginRemoveModal = ({
  isOpen,
  onClose,
  pluginName,
  onConfirm,
}: PluginRemoveModalProps) => {
  const { t } = useTranslation();

  return (
    <Modal isOpen={isOpen} onClose={onClose}>
      <div className="bg-elevated border border-strong rounded-xl shadow-2xl w-[480px] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-default bg-base">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-red-900/30 rounded-lg">
              <Trash2 size={20} className="text-red-400" />
            </div>
            <h2 className="text-lg font-semibold text-primary">
              {t("settings.plugins.removeTitle")}
            </h2>
          </div>
          <button onClick={onClose} className="text-secondary hover:text-primary transition-colors">
            <X size={20} />
          </button>
        </div>

        {/* Content */}
        <div className="p-6">
          <p className="text-sm text-secondary">
            {t("settings.plugins.confirmRemove", { name: pluginName })}
          </p>
        </div>

        {/* Footer */}
        <div className="p-4 border-t border-default bg-base/50 flex justify-end gap-3">
          <button
            onClick={onClose}
            className="px-4 py-2 text-secondary hover:text-primary transition-colors text-sm"
          >
            {t("common.cancel")}
          </button>
          <button
            onClick={onConfirm}
            className="px-4 py-2 bg-red-600 hover:bg-red-500 text-white rounded-lg text-sm font-medium transition-colors"
          >
            {t("settings.plugins.remove")}
          </button>
        </div>
      </div>
    </Modal>
  );
};
