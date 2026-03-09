import type { PluginConfig } from "../contexts/SettingsContext";

/**
 * Builds a PluginConfig from a raw interpreter string entered by the user.
 * Trims whitespace; an empty string clears the interpreter field.
 */
export function resolvePluginConfig(
  currentConfig: PluginConfig | undefined,
  rawInterpreter: string,
): PluginConfig {
  return {
    ...(currentConfig ?? {}),
    interpreter: rawInterpreter.trim() || undefined,
  };
}

/**
 * Returns the interpreter to display in the modal input for a given plugin.
 * Falls back to an empty string when none is configured.
 */
export function getDisplayInterpreter(config: PluginConfig | undefined): string {
  return config?.interpreter ?? "";
}
