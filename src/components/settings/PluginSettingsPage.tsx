import { useState, useCallback, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Check } from "lucide-react";
import { useSettings } from "../../hooks/useSettings";
import { useDrivers } from "../../hooks/useDrivers";
import { SettingSection, SettingRow } from "./SettingControls";
import { Select } from "../ui/Select";
import { SlotAnchor } from "../ui/SlotAnchor";
import {
  resolvePluginConfig,
  getDisplayInterpreter,
  resolveSettingsWithDefaults,
  validateSettings,
} from "../../utils/pluginConfig";
import type {
  PluginManifest,
  PluginSettingDefinition,
} from "../../types/plugins";

interface PluginSettingsPageProps {
  pluginId: string;
}

export function PluginSettingsPage({ pluginId }: PluginSettingsPageProps) {
  const { allDrivers } = useDrivers();
  const [manifest, setManifest] = useState<PluginManifest | undefined>(() =>
    allDrivers.find((d) => d.id === pluginId),
  );
  const [loading, setLoading] = useState(!manifest);

  useEffect(() => {
    if (manifest) return;
    invoke<PluginManifest>("get_plugin_manifest", { pluginId })
      .then((m) => setManifest(m))
      .catch(() => setManifest(undefined))
      .finally(() => setLoading(false));
  }, [pluginId, manifest]);

  if (loading) {
    return <div className="text-sm text-muted py-4">Loading...</div>;
  }

  return <PluginSettingsForm pluginId={pluginId} manifest={manifest} />;
}

/* ── Inner form (mounted fresh via key={pluginId} from parent) ── */

interface PluginSettingsFormProps {
  pluginId: string;
  manifest: PluginManifest | undefined;
}

function PluginSettingsForm({ pluginId, manifest }: PluginSettingsFormProps) {
  const { t } = useTranslation();
  const { settings, updateSetting } = useSettings();
  const { allDrivers, refresh: refreshDrivers } = useDrivers();

  const currentConfig = settings.plugins?.[pluginId];
  const [interpreter, setInterpreter] = useState(
    getDisplayInterpreter(currentConfig),
  );
  const definitions = manifest?.settings ?? [];
  const [dynamicValues, setDynamicValues] = useState<
    Record<string, unknown>
  >(() => resolveSettingsWithDefaults(definitions, currentConfig?.settings));
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [saved, setSaved] = useState(false);

  const handleBrowse = async () => {
    const selected = await open({ multiple: false, directory: false });
    if (selected) setInterpreter(selected);
  };

  const handleDynamicChange = useCallback(
    (key: string, value: unknown) => {
      setDynamicValues((prev) => ({ ...prev, [key]: value }));
      setErrors((prev) => {
        if (!prev[key]) return prev;
        const next = { ...prev };
        delete next[key];
        return next;
      });
      setSaved(false);
    },
    [],
  );

  const handleSave = useCallback(async () => {
    const validationErrors = validateSettings(definitions, dynamicValues);
    if (Object.keys(validationErrors).length > 0) {
      const errorMessages: Record<string, string> = {};
      for (const [key, label] of Object.entries(validationErrors)) {
        errorMessages[key] = t(
          "settings.plugins.pluginSettings.fieldRequired",
          { label },
        );
      }
      setErrors(errorMessages);
      return;
    }

    const baseConfig = resolvePluginConfig(currentConfig, interpreter);
    const mergedSettings =
      definitions.length > 0
        ? { ...(baseConfig.settings ?? {}), ...dynamicValues }
        : baseConfig.settings;

    const config = { ...baseConfig, settings: mergedSettings };
    const current = settings.plugins ?? {};
    updateSetting("plugins", { ...current, [pluginId]: config });

    const isRunning = allDrivers.some((d) => d.id === pluginId);
    if (isRunning) {
      try {
        await invoke("disable_plugin", { pluginId });
        await invoke("enable_plugin", { pluginId });
        refreshDrivers();
      } catch {
        /* settings saved, restart failed – user can retry via Plugins tab */
      }
    }

    setSaved(true);
  }, [
    definitions,
    dynamicValues,
    interpreter,
    currentConfig,
    settings.plugins,
    updateSetting,
    pluginId,
    allDrivers,
    refreshDrivers,
    t,
  ]);

  const renderField = (def: PluginSettingDefinition) => {
    const value = dynamicValues[def.key];
    const inputClass =
      "bg-base border-default text-primary placeholder:text-muted focus:border-blue-500/50 focus:outline-none";

    if (def.type === "boolean") {
      return (
        <input
          type="checkbox"
          checked={typeof value === "boolean" ? value : false}
          onChange={(e) =>
            handleDynamicChange(def.key, e.target.checked)
          }
          className="w-4 h-4 accent-blue-500"
        />
      );
    }

    if (def.type === "select" && def.options && def.options.length > 0) {
      return (
        <Select
          value={typeof value === "string" ? value : null}
          options={def.options}
          onChange={(v) => handleDynamicChange(def.key, v)}
          hasError={!!errors[def.key]}
        />
      );
    }

    if (def.type === "number") {
      return (
        <input
          type="number"
          value={typeof value === "number" ? value : ""}
          onChange={(e) =>
            handleDynamicChange(
              def.key,
              e.target.value === ""
                ? undefined
                : Number(e.target.value),
            )
          }
          className={`w-full border rounded-lg px-3 py-2 text-sm ${inputClass}`}
        />
      );
    }

    return (
      <input
        type="text"
        value={typeof value === "string" ? value : ""}
        onChange={(e) => handleDynamicChange(def.key, e.target.value)}
        className={`w-full border rounded-lg px-3 py-2 text-sm ${inputClass}`}
      />
    );
  };

  return (
    <div>
      {/* Header */}
      <div className="mb-6">
        <h2 className="text-lg font-semibold text-primary">
          {manifest?.name ?? pluginId}
        </h2>
        <p className="text-xs text-muted font-mono mt-0.5">{pluginId}</p>
      </div>

      {/* Interpreter */}
      <SettingSection
        title={t("settings.plugins.pluginSettings.interpreter")}
        description={t(
          "settings.plugins.pluginSettings.interpreterDesc",
        )}
      >
        <div className="py-3">
          <div className="flex gap-2">
            <input
              type="text"
              value={interpreter}
              placeholder={t(
                "settings.plugins.pluginSettings.interpreterPlaceholder",
              )}
              onChange={(e) => {
                setInterpreter(e.target.value);
                setSaved(false);
              }}
              className="flex-1 bg-base border border-default rounded-lg px-3 py-2 text-sm text-primary placeholder:text-muted focus:outline-none focus:border-blue-500/50"
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
      </SettingSection>

      {/* Plugin UI extension slot */}
      <SlotAnchor
        name="settings.plugin.before_settings"
        context={{ targetPluginId: pluginId }}
        className="flex flex-col gap-2 mb-6"
      />

      {/* Dynamic settings */}
      {definitions.length > 0 && (
        <SettingSection
          title={t("settings.plugins.pluginSettings.title")}
        >
          {definitions.map((def) => (
            <div key={def.key}>
              <SettingRow
                label={`${def.label}${def.required ? " *" : ""}`}
                description={def.description}
                vertical={def.type !== "boolean"}
              >
                {renderField(def)}
              </SettingRow>
              {errors[def.key] && (
                <p className="text-xs text-red-400 -mt-2 mb-2 pl-0.5">
                  {errors[def.key]}
                </p>
              )}
            </div>
          ))}
        </SettingSection>
      )}

      {/* Save */}
      <div className="flex items-center gap-3 pt-2">
        <button
          onClick={handleSave}
          className="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg text-sm font-medium transition-colors"
        >
          {t("common.save")}
        </button>
        {saved && (
          <span className="text-xs text-green-400 flex items-center gap-1">
            <Check size={12} />
            {t("settings.plugins.pluginSettings.saved")}
          </span>
        )}
      </div>
    </div>
  );
}
