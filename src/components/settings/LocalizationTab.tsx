import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { SUPPORTED_LANGUAGES, type AppLanguage } from "../../i18n/config";
import { SettingSection, SettingRow, SettingButtonGroup } from "./SettingControls";

export function LocalizationTab() {
  const { t } = useTranslation();
  const { settings, updateSetting } = useSettings();

  const options: Array<{ value: AppLanguage; label: string }> = [
    { value: "auto", label: t("settings.auto") },
    ...SUPPORTED_LANGUAGES.map(({ id, label }) => ({ value: id, label })),
  ];

  return (
    <div>
      <SettingSection title={t("settings.localization")}>
        <SettingRow
          label={t("settings.language")}
          description={t("settings.languageDesc")}
          vertical
        >
          <SettingButtonGroup
            value={settings.language ?? "auto"}
            onChange={(v) => updateSetting("language", v)}
            options={options}
          />
        </SettingRow>
      </SettingSection>
    </div>
  );
}
