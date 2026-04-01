import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Settings as SettingsIcon,
  Palette,
  Languages,
  Sparkles,
  ScrollText,
  Keyboard,
  Database,
  Info,
  FileJson,
} from "lucide-react";
import clsx from "clsx";
import { ConfigJsonModal } from "../components/modals/ConfigJsonModal";
import { GeneralTab } from "../components/settings/GeneralTab";
import { AppearanceTab } from "../components/settings/AppearanceTab";
import { LocalizationTab } from "../components/settings/LocalizationTab";
import { AiTab } from "../components/settings/AiTab";
import { LogsTab } from "../components/settings/LogsTab";
import { ShortcutsTab } from "../components/settings/ShortcutsTab";
import { PluginsTab } from "../components/settings/PluginsTab";
import { InfoTab } from "../components/settings/InfoTab";

type SettingsTab =
  | "general"
  | "appearance"
  | "localization"
  | "ai"
  | "logs"
  | "shortcuts"
  | "plugins"
  | "info";

const TAB_ITEMS: Array<{
  id: SettingsTab;
  icon: React.ComponentType<{ size: number }>;
  labelKey: string;
}> = [
  { id: "general", icon: SettingsIcon, labelKey: "settings.general" },
  { id: "appearance", icon: Palette, labelKey: "settings.appearance" },
  { id: "localization", icon: Languages, labelKey: "settings.localization" },
  { id: "ai", icon: Sparkles, labelKey: "settings.ai.tab" },
  { id: "logs", icon: ScrollText, labelKey: "settings.logs" },
  { id: "shortcuts", icon: Keyboard, labelKey: "settings.shortcuts.title" },
  { id: "plugins", icon: Database, labelKey: "settings.plugins.title" },
  { id: "info", icon: Info, labelKey: "settings.info" },
];

const TAB_COMPONENTS: Record<SettingsTab, React.ComponentType> = {
  general: GeneralTab,
  appearance: AppearanceTab,
  localization: LocalizationTab,
  ai: AiTab,
  logs: LogsTab,
  shortcuts: ShortcutsTab,
  plugins: PluginsTab,
  info: InfoTab,
};

export const Settings = () => {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<SettingsTab>("general");
  const [isConfigJsonModalOpen, setIsConfigJsonModalOpen] = useState(false);

  const ActiveComponent = TAB_COMPONENTS[activeTab];

  return (
    <div className="h-full flex bg-base">
      {/* Sidebar */}
      <nav className="w-52 flex flex-col border-r border-default bg-elevated shrink-0">
        <div className="flex-1 py-2 px-2 overflow-y-auto space-y-0.5">
          {TAB_ITEMS.map(({ id, icon: Icon, labelKey }) => (
            <button
              key={id}
              onClick={() => setActiveTab(id)}
              className={clsx(
                "w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm font-medium transition-colors text-left",
                activeTab === id
                  ? "bg-surface-secondary text-primary"
                  : "text-muted hover:text-primary hover:bg-surface-secondary/50",
              )}
            >
              <Icon size={16} />
              {t(labelKey)}
            </button>
          ))}
        </div>

        {/* Config JSON button */}
        <div className="p-2 border-t border-default">
          <button
            onClick={() => setIsConfigJsonModalOpen(true)}
            className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-xs text-muted hover:text-primary hover:bg-surface-secondary/50 transition-colors"
          >
            <FileJson size={14} />
            {t("settings.editConfigJson")}
          </button>
        </div>
      </nav>

      {/* Content */}
      <div className="flex-1 overflow-auto">
        <div className="max-w-5xl mx-auto p-8">
          <ActiveComponent />
        </div>
      </div>

      <ConfigJsonModal
        isOpen={isConfigJsonModalOpen}
        onClose={() => setIsConfigJsonModalOpen(false)}
      />
    </div>
  );
};
