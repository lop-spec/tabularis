import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Settings, X, FolderOpen } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { Modal } from "../ui/Modal";
import { resolvePluginConfig, getDisplayInterpreter } from "../../utils/pluginConfig";
import type { PluginConfig } from "../../contexts/SettingsContext";

interface PluginSettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  pluginId: string;
  pluginName: string;
  currentConfig?: PluginConfig;
  onSave: (config: PluginConfig) => void;
}

export const PluginSettingsModal = ({
  isOpen,
  onClose,
  pluginId,
  currentConfig,
  onSave,
}: PluginSettingsModalProps) => {
  const { t } = useTranslation();
  const [interpreter, setInterpreter] = useState(getDisplayInterpreter(currentConfig));

  const handleBrowse = async () => {
    const selected = await open({ multiple: false, directory: false });
    if (selected) setInterpreter(selected);
  };

  const handleSave = () => {
    onSave(resolvePluginConfig(currentConfig, interpreter));
    onClose();
  };

  return (
    <Modal isOpen={isOpen} onClose={onClose}>
      <div className="bg-elevated border border-strong rounded-xl shadow-2xl w-[600px] max-h-[90vh] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-default bg-base">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-blue-900/30 rounded-lg">
              <Settings size={20} className="text-blue-400" />
            </div>
            <div>
              <h2 className="text-lg font-semibold text-primary">
                {t("settings.plugins.pluginSettings.title")}
              </h2>
              <p className="text-xs text-secondary font-mono">{pluginId}</p>
            </div>
          </div>
          <button onClick={onClose} className="text-secondary hover:text-primary transition-colors">
            <X size={20} />
          </button>
        </div>

        {/* Content */}
        <div className="p-6 space-y-6 overflow-y-auto">
          <div className="space-y-2">
            <label className="text-sm font-medium text-primary">
              {t("settings.plugins.pluginSettings.interpreter")}
            </label>
            <p className="text-xs text-secondary">
              {t("settings.plugins.pluginSettings.interpreterDesc")}
            </p>
            <div className="flex gap-2">
              <input
                type="text"
                value={interpreter}
                placeholder={t("settings.plugins.pluginSettings.interpreterPlaceholder")}
                onChange={(e) => setInterpreter(e.target.value)}
                className="flex-1 bg-base border border-default rounded-lg px-3 py-2 text-sm text-primary placeholder:text-muted focus:outline-none focus:border-blue-500/50"
                autoFocus
              />
              <button
                onClick={handleBrowse}
                className="flex items-center gap-1.5 px-3 py-2 bg-surface-secondary hover:bg-surface-tertiary border border-default rounded-lg text-sm text-secondary hover:text-primary transition-colors"
              >
                <FolderOpen size={15} />
                {t("settings.plugins.pluginSettings.browse")}
              </button>
            </div>
          </div>
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
            onClick={handleSave}
            className="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg text-sm font-medium transition-colors"
          >
            {t("common.save")}
          </button>
        </div>
      </div>
    </Modal>
  );
};
