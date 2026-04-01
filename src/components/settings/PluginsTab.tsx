import { useState, useCallback, useEffect, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import { invoke } from "@tauri-apps/api/core";
import {
  RefreshCw,
  Loader2,
  AlertTriangle,
  Download,
  RotateCcw,
  Check,
  ExternalLink,
  Settings as SettingsIcon,
  Trash2,
  ChevronDown,
  CheckCircle2,
} from "lucide-react";
import clsx from "clsx";
import { useSettings } from "../../hooks/useSettings";
import { useDrivers } from "../../hooks/useDrivers";
import { usePluginRegistry } from "../../hooks/usePluginRegistry";
import { useDatabase } from "../../hooks/useDatabase";
import { SettingSection } from "./SettingControls";
import { parseAuthor, versionGte } from "../../utils/plugins";
import { findConnectionsForDrivers } from "../../utils/connectionManager";
import { APP_VERSION } from "../../version";
import type { PluginManifest } from "../../types/plugins";
import type { PluginConfig } from "../../contexts/SettingsContext";
import { PluginInstallErrorModal } from "../modals/PluginInstallErrorModal";
import { PluginRemoveModal } from "../modals/PluginRemoveModal";
import { PluginSettingsModal } from "../modals/PluginSettingsModal";
import { PluginStartErrorModal } from "../modals/PluginStartErrorModal";
import { SlotAnchor } from "../ui/SlotAnchor";

/* ── Plugin card ── */

interface PluginCardProps {
  name: string;
  description: string;
  version?: string;
  author?: string;
  homepage?: string;
  status?: ReactNode;
  actions: ReactNode;
  dimmed?: boolean;
}

function PluginCard({
  name,
  description,
  version,
  author,
  homepage,
  status,
  actions,
  dimmed,
}: PluginCardProps) {
  const { t } = useTranslation();
  const parsedAuthor = author ? parseAuthor(author) : null;
  return (
    <div
      className={`grid grid-cols-[1fr_auto] gap-x-6 px-5 py-4 rounded-xl border border-surface-tertiary bg-surface-secondary transition-colors hover:border-surface-quaternary${dimmed ? " opacity-50" : ""}`}
    >
      <div className="min-w-0">
        <div className="flex items-baseline gap-2 flex-wrap">
          <span className="text-sm font-semibold text-primary">
            {homepage ? (
              <button
                type="button"
                onClick={() => openUrl(homepage)}
                className="hover:underline inline-flex items-center gap-1 text-primary"
              >
                {name}
                <ExternalLink
                  size={10}
                  className="text-muted shrink-0"
                />
              </button>
            ) : (
              name
            )}
          </span>
          {version && (
            <span className="text-[10px] font-mono text-muted bg-surface-tertiary px-1.5 py-px rounded">
              v{version}
            </span>
          )}
          {status}
        </div>
        <p className="text-xs text-secondary mt-1 leading-relaxed line-clamp-2">
          {description}
        </p>
        {parsedAuthor && (
          <p className="text-[10px] text-muted mt-1.5">
            {t("settings.plugins.by")}{" "}
            {parsedAuthor.url ?? homepage ? (
              <button
                type="button"
                onClick={() => openUrl((parsedAuthor.url ?? homepage)!)}
                className="hover:text-secondary underline-offset-2 hover:underline transition-colors"
              >
                {parsedAuthor.name}
              </button>
            ) : (
              parsedAuthor.name
            )}
          </p>
        )}
      </div>
      <div className="flex flex-col items-end justify-center gap-2 shrink-0 min-w-[160px]">
        {actions}
      </div>
    </div>
  );
}

/* ── Version dropdown ── */

interface VersionOption {
  version: string;
  isInstalled: boolean;
  isLatest: boolean;
}

function VersionDropdown({
  options,
  value,
  onChange,
  isDowngrade,
  label,
}: {
  options: VersionOption[];
  value: string;
  onChange: (v: string) => void;
  isDowngrade: boolean;
  label: string;
}) {
  const [isOpen, setIsOpen] = useState(false);
  const [pos, setPos] = useState({ top: 0, left: 0, minWidth: 0 });
  const btnRef = useRef<HTMLButtonElement>(null);
  const dropRef = useRef<HTMLDivElement>(null);

  const updatePos = () => {
    if (btnRef.current) {
      const r = btnRef.current.getBoundingClientRect();
      setPos({
        top: r.bottom + 4,
        left: r.left,
        minWidth: Math.max(r.width, 160),
      });
    }
  };

  useEffect(() => {
    if (!isOpen) return;
    const onMouseDown = (e: MouseEvent) => {
      if (
        !btnRef.current?.contains(e.target as Node) &&
        !dropRef.current?.contains(e.target as Node)
      ) {
        setIsOpen(false);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setIsOpen(false);
    };
    document.addEventListener("mousedown", onMouseDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onMouseDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [isOpen]);

  return (
    <>
      <button
        ref={btnRef}
        type="button"
        onClick={() => {
          updatePos();
          setIsOpen((o) => !o);
        }}
        className={clsx(
          "flex items-center gap-1.5 px-2.5 py-1 rounded-md border text-[11px] bg-surface-tertiary transition-colors cursor-pointer select-none",
          isDowngrade
            ? "border-amber-500/30 text-amber-400/80 hover:border-amber-500/60 hover:text-amber-400"
            : isOpen
              ? "border-blue-500/60 text-primary"
              : "border-surface-quaternary text-secondary hover:border-blue-500/50 hover:text-primary",
        )}
      >
        <RotateCcw size={9} />
        <span>{label}</span>
        <ChevronDown
          size={9}
          className={clsx(
            "transition-transform duration-150",
            isOpen && "rotate-180",
          )}
        />
      </button>

      {isOpen &&
        createPortal(
          <div
            ref={dropRef}
            style={{
              top: pos.top,
              left: pos.left,
              minWidth: pos.minWidth,
            }}
            className="fixed z-[200] bg-elevated border border-strong rounded-lg shadow-xl overflow-hidden"
          >
            {options.map((opt) => (
              <button
                key={opt.version}
                type="button"
                onClick={() => {
                  onChange(opt.version);
                  setIsOpen(false);
                }}
                className={clsx(
                  "w-full flex items-center gap-2 px-3 py-2 text-xs text-left transition-colors",
                  opt.isInstalled
                    ? "bg-green-500/10 hover:bg-green-500/20"
                    : opt.version === value
                      ? "bg-surface-secondary"
                      : "hover:bg-surface-secondary",
                )}
              >
                <span className="w-3 shrink-0 flex items-center justify-center">
                  {opt.isInstalled && (
                    <Check size={10} className="text-green-400" />
                  )}
                </span>
                <span
                  className={clsx(
                    "font-mono",
                    opt.isInstalled
                      ? "text-green-300"
                      : "text-primary",
                  )}
                >
                  v{opt.version}
                </span>
                <span className="ml-auto flex items-center gap-1">
                  {opt.isInstalled && (
                    <span className="text-[9px] font-medium bg-green-500/20 text-green-400 px-1.5 py-px rounded">
                      installed
                    </span>
                  )}
                  {opt.isLatest && (
                    <span className="text-[9px] font-medium bg-blue-500/20 text-blue-400 px-1.5 py-px rounded">
                      latest
                    </span>
                  )}
                </span>
              </button>
            ))}
          </div>,
          document.body,
        )}
    </>
  );
}

/* ── Main tab ── */

export function PluginsTab() {
  const { t } = useTranslation();
  const { settings, updateSetting } = useSettings();
  const {
    allDrivers,
    installedPlugins,
    refresh: refreshDrivers,
  } = useDrivers();
  const {
    plugins: registryPlugins,
    loading: registryLoading,
    error: registryError,
    refresh: refreshRegistry,
  } = usePluginRegistry();
  const { openConnectionIds, connectionDataMap, disconnect } = useDatabase();

  const [installingPluginId, setInstallingPluginId] = useState<string | null>(
    null,
  );
  const [pluginInstallError, setPluginInstallError] = useState<{
    pluginId: string;
    error: string;
  } | null>(null);
  const [pluginSettingsModal, setPluginSettingsModal] = useState<{
    pluginId: string;
    pluginName: string;
  } | null>(null);
  const [openModalManifest, setOpenModalManifest] = useState<
    PluginManifest | undefined
  >(undefined);
  const [pluginStartError, setPluginStartError] = useState<{
    pluginId: string;
    pluginName: string;
    error: string;
  } | null>(null);
  const [selectedVersions, setSelectedVersions] = useState<
    Record<string, string>
  >({});
  const [uninstallingPluginId, setUninstallingPluginId] = useState<
    string | null
  >(null);
  const [pluginRemoveConfirm, setPluginRemoveConfirm] = useState<{
    pluginId: string;
    pluginName: string;
    onConfirm: () => Promise<void>;
  } | null>(null);

  const settingsRef = useRef(settings);
  settingsRef.current = settings;
  const updateSettingRef = useRef(updateSetting);
  updateSettingRef.current = updateSetting;

  useEffect(() => {
    invoke<Array<{ plugin_id: string; error: string }>>(
      "get_plugin_startup_errors",
    )
      .then((errors) => {
        if (errors.length > 0) {
          const failedIds = errors.map((e) => e.plugin_id);
          const activeExt =
            settingsRef.current.activeExternalDrivers ?? [];
          const cleaned = activeExt.filter(
            (id) => !failedIds.includes(id),
          );
          if (cleaned.length !== activeExt.length) {
            updateSettingRef.current("activeExternalDrivers", cleaned);
          }
          const first = errors[0];
          setPluginStartError({
            pluginId: first.plugin_id,
            pluginName: first.plugin_id,
            error: first.error,
          });
        }
      })
      .catch(() => {
        /* ignore */
      });
  }, []);

  const handleSavePluginConfig = useCallback(
    async (
      pluginId: string,
      pluginName: string,
      config: PluginConfig,
    ) => {
      const current = settings.plugins ?? {};
      updateSetting("plugins", { ...current, [pluginId]: config });
      const isRunning = allDrivers.some((d) => d.id === pluginId);
      if (isRunning) {
        try {
          await invoke("disable_plugin", { pluginId });
          await invoke("enable_plugin", { pluginId });
          refreshDrivers();
        } catch (err) {
          const activeExt = settings.activeExternalDrivers ?? [];
          updateSetting(
            "activeExternalDrivers",
            activeExt.filter((id) => id !== pluginId),
          );
          refreshDrivers();
          setPluginStartError({
            pluginId,
            pluginName,
            error: String(err),
          });
        }
      }
    },
    [
      settings.plugins,
      settings.activeExternalDrivers,
      updateSetting,
      allDrivers,
      refreshDrivers,
    ],
  );

  const handleOpenPluginSettings = useCallback(
    async (pluginId: string, pluginName: string) => {
      setPluginSettingsModal({ pluginId, pluginName });
      const runningDriver = allDrivers.find((d) => d.id === pluginId);
      if (runningDriver) {
        setOpenModalManifest(runningDriver);
      } else {
        try {
          const manifest = await invoke<PluginManifest>(
            "get_plugin_manifest",
            { pluginId },
          );
          setOpenModalManifest(manifest);
        } catch {
          setOpenModalManifest(undefined);
        }
      }
    },
    [allDrivers],
  );

  const doInstall = useCallback(
    async (pluginId: string, version: string) => {
      setInstallingPluginId(pluginId);
      try {
        await invoke("install_plugin", { pluginId, version });
        refreshRegistry();
        refreshDrivers();
      } catch (err) {
        setPluginInstallError({ pluginId, error: String(err) });
      } finally {
        setInstallingPluginId(null);
      }
    },
    [refreshRegistry, refreshDrivers],
  );

  const doRemove = useCallback(
    (pluginId: string, pluginName: string) => {
      setPluginRemoveConfirm({
        pluginId,
        pluginName,
        onConfirm: async () => {
          setUninstallingPluginId(pluginId);
          setPluginRemoveConfirm(null);
          try {
            const toDisconnect = findConnectionsForDrivers(
              openConnectionIds,
              connectionDataMap,
              [pluginId],
            );
            await Promise.all(toDisconnect.map((id) => disconnect(id)));
            await invoke("uninstall_plugin", { pluginId });
            refreshDrivers();
            refreshRegistry();
          } catch (err) {
            setPluginInstallError({ pluginId, error: String(err) });
          } finally {
            setUninstallingPluginId(null);
          }
        },
      });
    },
    [
      openConnectionIds,
      connectionDataMap,
      disconnect,
      refreshDrivers,
      refreshRegistry,
    ],
  );

  const doToggle = useCallback(
    async (
      pluginId: string,
      pluginName: string,
      isEnabled: boolean,
    ) => {
      const activeExt = settings.activeExternalDrivers || [];
      try {
        if (isEnabled) {
          await invoke("disable_plugin", { pluginId });
          updateSetting(
            "activeExternalDrivers",
            activeExt.filter((id) => id !== pluginId),
          );
        } else {
          await invoke("enable_plugin", { pluginId });
          updateSetting("activeExternalDrivers", [
            ...activeExt,
            pluginId,
          ]);
        }
        refreshDrivers();
      } catch (err) {
        setPluginStartError({
          pluginId,
          pluginName,
          error: String(err),
        });
      }
    },
    [settings.activeExternalDrivers, updateSetting, refreshDrivers],
  );

  return (
    <>
      <div>
        {/* Available */}
        <SettingSection
          title={t("settings.plugins.available")}
          description={t("settings.plugins.availableDesc")}
          action={
            <button
              onClick={() => refreshRegistry()}
              className="text-xs text-muted hover:text-primary flex items-center gap-1 transition-colors"
            >
              <RefreshCw size={12} />
              {t("settings.plugins.refresh")}
            </button>
          }
        >
        <div className="pt-3">
          {registryLoading && (
            <div className="flex items-center gap-2 text-muted text-sm py-4">
              <Loader2 size={16} className="animate-spin" />
              {t("settings.plugins.loadingRegistry")}
            </div>
          )}

          {registryError && (
            <div className="bg-red-900/20 border border-red-900/50 text-red-400 px-4 py-3 rounded-lg text-sm flex items-center gap-2">
              <AlertTriangle size={16} />
              {t("settings.plugins.registryError")}: {registryError}
            </div>
          )}

          {!registryLoading && !registryError && (
            <div className="space-y-3">
              {registryPlugins.map((plugin) => {
                const platformReleases = plugin.releases.filter(
                  (r) => r.platform_supported,
                );
                const installableReleases = platformReleases.filter(
                  (r) => r.version !== plugin.installed_version,
                );
                const isAtLatest =
                  !!plugin.installed_version &&
                  plugin.installed_version === plugin.latest_version;
                const defaultVer = isAtLatest
                  ? plugin.latest_version
                  : (installableReleases.find(
                      (r) => r.version === plugin.latest_version,
                    )?.version ??
                    installableReleases[0]?.version ??
                    plugin.latest_version);
                const selectedVer =
                  selectedVersions[plugin.id] ?? defaultVer;
                const selectedRelease = plugin.releases.find(
                  (r) => r.version === selectedVer,
                );
                const selectedPlatformSupported =
                  selectedRelease?.platform_supported ?? false;
                const isSelectedInstalled =
                  plugin.installed_version === selectedVer;
                const minVersion =
                  selectedRelease?.min_tabularis_version ?? null;
                const isCompatible =
                  !minVersion || versionGte(APP_VERSION, minVersion);
                const isUpdate =
                  !!plugin.installed_version && !isSelectedInstalled;
                const isDowngrade =
                  isUpdate &&
                  !versionGte(selectedVer, plugin.installed_version!);
                const showVersionPicker = isAtLatest
                  ? installableReleases.length >= 1
                  : installableReleases.length > 1;

                const installedBadge = plugin.installed_version ? (
                  <span className="text-[10px] font-medium bg-green-500/10 text-green-400 border border-green-500/20 px-1.5 py-px rounded-md">
                    {t("settings.plugins.installed")} v
                    {plugin.installed_version}
                  </span>
                ) : undefined;

                return (
                  <PluginCard
                    key={plugin.id}
                    name={plugin.name}
                    description={plugin.description}
                    author={plugin.author}
                    homepage={plugin.homepage}
                    status={installedBadge}
                    actions={
                      !selectedPlatformSupported ? (
                        <span className="text-xs text-muted italic text-right">
                          {t("settings.plugins.platformNotSupported")}
                        </span>
                      ) : (
                        <>
                          {isSelectedInstalled &&
                            selectedVer ===
                              plugin.latest_version && (
                              <div className="flex items-center justify-center gap-1.5">
                                <CheckCircle2
                                  size={12}
                                  className="text-green-400"
                                />
                                <span className="text-xs text-green-400 font-medium">
                                  {t("settings.plugins.upToDate")}
                                </span>
                              </div>
                            )}

                          {!isSelectedInstalled &&
                            (isCompatible ? (
                              <button
                                onClick={() =>
                                  doInstall(plugin.id, selectedVer)
                                }
                                disabled={
                                  installingPluginId === plugin.id
                                }
                                className={`w-full flex items-center justify-center gap-1.5 px-4 py-1.5 rounded-lg text-xs font-medium text-white transition-colors disabled:opacity-50 ${
                                  isDowngrade
                                    ? "bg-amber-600 hover:bg-amber-500"
                                    : isUpdate
                                      ? "bg-green-600 hover:bg-green-500"
                                      : "bg-blue-600 hover:bg-blue-500"
                                }`}
                              >
                                {installingPluginId === plugin.id ? (
                                  <Loader2
                                    size={12}
                                    className="animate-spin"
                                  />
                                ) : isDowngrade ? (
                                  <RotateCcw size={12} />
                                ) : isUpdate ? (
                                  <RefreshCw size={12} />
                                ) : (
                                  <Download size={12} />
                                )}
                                {isDowngrade
                                  ? `${t("settings.plugins.downgrade")} v${selectedVer}`
                                  : isUpdate
                                    ? `${t("settings.plugins.update")} v${selectedVer}`
                                    : `${t("settings.plugins.install")} v${selectedVer}`}
                              </button>
                            ) : (
                              <div className="w-full flex flex-col items-end gap-1">
                                <button
                                  disabled
                                  title={t(
                                    "settings.plugins.requiresVersion",
                                    { version: minVersion },
                                  )}
                                  className="w-full flex items-center justify-center gap-1.5 px-4 py-1.5 rounded-lg text-xs font-medium text-muted bg-surface-tertiary cursor-not-allowed opacity-50"
                                >
                                  <Download size={12} />
                                  {t("settings.plugins.install")} v
                                  {selectedVer}
                                </button>
                                <span className="text-[10px] text-amber-400/80 text-right">
                                  {t(
                                    "settings.plugins.requiresVersion",
                                    { version: minVersion },
                                  )}
                                </span>
                              </div>
                            ))}

                          {showVersionPicker &&
                            (() => {
                              const dropdownOptions: VersionOption[] = [
                                ...(isAtLatest
                                  ? [
                                      {
                                        version:
                                          plugin.latest_version!,
                                        isInstalled: true,
                                        isLatest: true,
                                      },
                                    ]
                                  : []),
                                ...[...installableReleases]
                                  .reverse()
                                  .map((r) => ({
                                    version: r.version,
                                    isInstalled: false,
                                    isLatest:
                                      r.version ===
                                      plugin.latest_version,
                                  })),
                              ];
                              return (
                                <VersionDropdown
                                  options={dropdownOptions}
                                  value={selectedVer}
                                  onChange={(v) =>
                                    setSelectedVersions((prev) => ({
                                      ...prev,
                                      [plugin.id]: v,
                                    }))
                                  }
                                  isDowngrade={isDowngrade}
                                  label={
                                    isAtLatest && isSelectedInstalled
                                      ? t(
                                          "settings.plugins.olderVersions",
                                        )
                                      : `v${selectedVer}`
                                  }
                                />
                              );
                            })()}
                        </>
                      )
                    }
                  />
                );
              })}
              {registryPlugins.length === 0 && (
                <p className="text-sm text-muted py-4">
                  {t("settings.plugins.noPlugins")}
                </p>
              )}
            </div>
          )}
        </div>
        </SettingSection>

        {/* Installed */}
        <SettingSection
          title={t("settings.plugins.installedPlugins")}
          description={t("settings.plugins.installedDesc")}
        >
          <div className="space-y-3 pt-3">
            {allDrivers.map((driver: PluginManifest) => {
              const isBuiltin = driver.is_builtin === true;
              const registryPlugin = registryPlugins.find(
                (p) => p.id === driver.id,
              );
              const activeExt = settings.activeExternalDrivers || [];
              const isEnabled =
                isBuiltin || activeExt.includes(driver.id);

              const builtinBadge = isBuiltin ? (
                <span className="text-[10px] font-medium bg-blue-500/10 text-blue-400 border border-blue-500/20 px-1.5 py-px rounded-md">
                  Built-in
                </span>
              ) : undefined;

              return (
                <PluginCard
                  key={driver.id}
                  name={driver.name}
                  description={driver.description}
                  version={driver.version}
                  author={
                    !isBuiltin ? registryPlugin?.author : undefined
                  }
                  homepage={
                    !isBuiltin ? registryPlugin?.homepage : undefined
                  }
                  dimmed={isBuiltin}
                  status={builtinBadge}
                  actions={
                    <div className="flex flex-col items-end gap-2 w-full">
                      <button
                        onClick={() =>
                          !isBuiltin &&
                          doToggle(
                            driver.id,
                            driver.name,
                            isEnabled,
                          )
                        }
                        disabled={isBuiltin}
                        aria-label={
                          isEnabled
                            ? "Disable plugin"
                            : "Enable plugin"
                        }
                        className={`relative inline-flex h-5 w-9 shrink-0 rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 ${
                          isEnabled
                            ? "bg-blue-600"
                            : "bg-surface-tertiary"
                        } ${isBuiltin ? "cursor-not-allowed" : "cursor-pointer"}`}
                      >
                        <span
                          className={`pointer-events-none inline-block h-4 w-4 rounded-full bg-white shadow transition duration-200 ease-in-out ${isEnabled ? "translate-x-4" : "translate-x-0"}`}
                        />
                      </button>
                      {!isBuiltin && (
                        <button
                          onClick={() =>
                            handleOpenPluginSettings(
                              driver.id,
                              driver.name,
                            )
                          }
                          className="p-1.5 text-secondary hover:text-primary transition-colors"
                          title={t(
                            "settings.plugins.pluginSettings.title",
                          )}
                        >
                          <SettingsIcon size={15} />
                        </button>
                      )}
                      {!isBuiltin && (
                        <SlotAnchor
                          name="settings.plugin.actions"
                          context={{ targetPluginId: driver.id }}
                          className="flex flex-col items-end gap-1"
                        />
                      )}
                      {!isBuiltin && (
                        <button
                          onClick={() =>
                            doRemove(driver.id, driver.name)
                          }
                          disabled={
                            uninstallingPluginId === driver.id
                          }
                          className="flex items-center gap-1 text-[11px] text-red-500/70 hover:text-red-400 disabled:opacity-50 transition-colors"
                        >
                          {uninstallingPluginId === driver.id ? (
                            <Loader2
                              size={11}
                              className="animate-spin"
                            />
                          ) : (
                            <Trash2 size={11} />
                          )}
                          {t("settings.plugins.remove")}
                        </button>
                      )}
                    </div>
                  }
                />
              );
            })}

            {/* Disabled external plugins */}
            {installedPlugins
              .filter((p) => !allDrivers.some((d) => d.id === p.id))
              .map((plugin) => {
                const activeExt =
                  settings.activeExternalDrivers || [];
                const registryPlugin = registryPlugins.find(
                  (r) => r.id === plugin.id,
                );
                return (
                  <PluginCard
                    key={plugin.id}
                    name={plugin.name}
                    description={plugin.description}
                    version={plugin.version}
                    author={registryPlugin?.author}
                    homepage={registryPlugin?.homepage}
                    actions={
                      <div className="flex flex-col items-end gap-2 w-full">
                        <button
                          onClick={async () => {
                            try {
                              await invoke("enable_plugin", {
                                pluginId: plugin.id,
                              });
                              updateSetting(
                                "activeExternalDrivers",
                                [...activeExt, plugin.id],
                              );
                              refreshDrivers();
                            } catch (err) {
                              setPluginStartError({
                                pluginId: plugin.id,
                                pluginName: plugin.name,
                                error: String(err),
                              });
                            }
                          }}
                          aria-label="Enable plugin"
                          className="relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent bg-surface-tertiary transition-colors duration-200 ease-in-out focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
                        >
                          <span className="pointer-events-none inline-block h-4 w-4 rounded-full bg-white shadow transition duration-200 ease-in-out translate-x-0" />
                        </button>
                        <button
                          onClick={() =>
                            handleOpenPluginSettings(
                              plugin.id,
                              plugin.name,
                            )
                          }
                          className="p-1.5 text-secondary hover:text-primary transition-colors"
                          title={t(
                            "settings.plugins.pluginSettings.title",
                          )}
                        >
                          <SettingsIcon size={15} />
                        </button>
                        <SlotAnchor
                          name="settings.plugin.actions"
                          context={{ targetPluginId: plugin.id }}
                          className="flex flex-col items-end gap-1"
                        />
                        <button
                          onClick={() =>
                            doRemove(plugin.id, plugin.name)
                          }
                          disabled={
                            uninstallingPluginId === plugin.id
                          }
                          className="flex items-center gap-1 text-[11px] text-red-500/70 hover:text-red-400 disabled:opacity-50 transition-colors"
                        >
                          {uninstallingPluginId === plugin.id ? (
                            <Loader2
                              size={11}
                              className="animate-spin"
                            />
                          ) : (
                            <Trash2 size={11} />
                          )}
                          {t("settings.plugins.remove")}
                        </button>
                      </div>
                    }
                  />
                );
              })}
          </div>
        </SettingSection>
      </div>

      {/* Modals */}
      <PluginInstallErrorModal
        isOpen={pluginInstallError !== null}
        onClose={() => setPluginInstallError(null)}
        pluginId={pluginInstallError?.pluginId ?? ""}
        error={pluginInstallError?.error ?? ""}
      />
      <PluginRemoveModal
        isOpen={pluginRemoveConfirm !== null}
        onClose={() => setPluginRemoveConfirm(null)}
        pluginName={pluginRemoveConfirm?.pluginName ?? ""}
        onConfirm={() => pluginRemoveConfirm?.onConfirm()}
      />
      <PluginSettingsModal
        key={pluginSettingsModal?.pluginId}
        isOpen={pluginSettingsModal !== null}
        onClose={() => {
          setPluginSettingsModal(null);
          setOpenModalManifest(undefined);
        }}
        pluginId={pluginSettingsModal?.pluginId ?? ""}
        pluginName={pluginSettingsModal?.pluginName ?? ""}
        currentConfig={
          settings.plugins?.[pluginSettingsModal?.pluginId ?? ""]
        }
        manifest={openModalManifest}
        onSave={(config) =>
          handleSavePluginConfig(
            pluginSettingsModal?.pluginId ?? "",
            pluginSettingsModal?.pluginName ?? "",
            config,
          )
        }
      />
      <PluginStartErrorModal
        isOpen={pluginStartError !== null}
        onClose={() => setPluginStartError(null)}
        pluginId={pluginStartError?.pluginId ?? ""}
        error={pluginStartError?.error ?? ""}
      />
    </>
  );
}
