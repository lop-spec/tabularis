import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Archive,
  Cable,
  FileJson,
  Info,
  Keyboard,
  Languages,
  Palette,
  ScrollText,
  Settings as SettingsIcon,
} from "lucide-react";
import clsx from "clsx";
import { ConfigJsonModal } from "../components/modals/ConfigJsonModal";
import { AppearanceTab } from "../components/settings/AppearanceTab";
import { BackupTab } from "../components/settings/BackupTab";
import { GeneralTab } from "../components/settings/GeneralTab";
import { InfoTab } from "../components/settings/InfoTab";
import { LocalizationTab } from "../components/settings/LocalizationTab";
import { LogsTab } from "../components/settings/LogsTab";
import { ShortcutsTab } from "../components/settings/ShortcutsTab";
import { SshTab } from "../components/settings/SshTab";

type SettingsTab =
  | "general"
  | "ssh"
  | "backup"
  | "appearance"
  | "localization"
  | "logs"
  | "shortcuts"
  | "info";

const TAB_ITEMS: Array<{
  id: SettingsTab;
  icon: React.ComponentType<{ size: number }>;
  labelKey: string;
}> = [
  { id: "general", icon: SettingsIcon, labelKey: "settings.general" },
  { id: "ssh", icon: Cable, labelKey: "sshConnections.title" },
  { id: "backup", icon: Archive, labelKey: "settings.backup.title" },
  { id: "appearance", icon: Palette, labelKey: "settings.appearance" },
  { id: "localization", icon: Languages, labelKey: "settings.localization" },
  { id: "logs", icon: ScrollText, labelKey: "settings.logs" },
  { id: "shortcuts", icon: Keyboard, labelKey: "settings.shortcuts.title" },
  { id: "info", icon: Info, labelKey: "settings.info" },
];

const TAB_COMPONENTS: Record<SettingsTab, React.ComponentType> = {
  general: GeneralTab,
  ssh: SshTab,
  backup: BackupTab,
  appearance: AppearanceTab,
  localization: LocalizationTab,
  logs: LogsTab,
  shortcuts: ShortcutsTab,
  info: InfoTab,
};

export const Settings = () => {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<SettingsTab>("general");
  const [isConfigJsonModalOpen, setIsConfigJsonModalOpen] = useState(false);
  const ActiveComponent = TAB_COMPONENTS[activeTab];

  return (
    <div className="h-full flex bg-base">
      <nav className="w-52 flex flex-col border-r border-default bg-elevated shrink-0">
        <div className="flex-1 py-2 px-2 overflow-y-auto space-y-0.5">
          {TAB_ITEMS.map(({ id, icon: Icon, labelKey }) => (
            <button
              key={id}
              type="button"
              onClick={() => setActiveTab(id)}
              className={clsx(
                "w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors",
                activeTab === id
                  ? "bg-accent text-white"
                  : "text-secondary hover:bg-surface-secondary hover:text-primary",
              )}
            >
              <Icon size={16} />
              <span>{t(labelKey)}</span>
            </button>
          ))}
        </div>
        <div className="p-2 border-t border-default">
          <button
            type="button"
            onClick={() => setIsConfigJsonModalOpen(true)}
            className="w-full flex items-center gap-3 px-3 py-2 rounded-lg text-xs text-muted hover:bg-surface-secondary hover:text-primary transition-colors"
          >
            <FileJson size={15} />
            <span>{t("settings.editConfigJson")}</span>
          </button>
        </div>
      </nav>

      <main className="flex-1 min-w-0 overflow-y-auto">
        <ActiveComponent />
      </main>

      <ConfigJsonModal
        isOpen={isConfigJsonModalOpen}
        onClose={() => setIsConfigJsonModalOpen(false)}
      />
    </div>
  );
};
