import { describe, it, expect } from "vitest";
import { resolvePluginConfig, getDisplayInterpreter } from "../../src/utils/pluginConfig";
import type { PluginConfig } from "../../src/contexts/SettingsContext";

describe("pluginConfig", () => {
  describe("resolvePluginConfig", () => {
    it("sets interpreter when a non-empty string is provided", () => {
      const result = resolvePluginConfig(undefined, "python3");
      expect(result.interpreter).toBe("python3");
    });

    it("trims whitespace from interpreter", () => {
      const result = resolvePluginConfig(undefined, "  python  ");
      expect(result.interpreter).toBe("python");
    });

    it("sets interpreter to undefined when empty string is provided", () => {
      const result = resolvePluginConfig(undefined, "");
      expect(result.interpreter).toBeUndefined();
    });

    it("sets interpreter to undefined when only whitespace is provided", () => {
      const result = resolvePluginConfig(undefined, "   ");
      expect(result.interpreter).toBeUndefined();
    });

    it("preserves existing settings when updating interpreter", () => {
      const current: PluginConfig = {
        interpreter: "old-python",
        settings: { timeout: 30 },
      };
      const result = resolvePluginConfig(current, "python3");
      expect(result.interpreter).toBe("python3");
      expect(result.settings).toEqual({ timeout: 30 });
    });

    it("clears interpreter while preserving other settings", () => {
      const current: PluginConfig = {
        interpreter: "python3",
        settings: { timeout: 30 },
      };
      const result = resolvePluginConfig(current, "");
      expect(result.interpreter).toBeUndefined();
      expect(result.settings).toEqual({ timeout: 30 });
    });

    it("accepts a full path interpreter", () => {
      const result = resolvePluginConfig(undefined, "C:\\Python311\\python.exe");
      expect(result.interpreter).toBe("C:\\Python311\\python.exe");
    });

    it("starts from empty config when currentConfig is undefined", () => {
      const result = resolvePluginConfig(undefined, "python3");
      expect(result).toEqual({ interpreter: "python3" });
    });

    it("starts from empty config when currentConfig has no settings", () => {
      const current: PluginConfig = { interpreter: "python" };
      const result = resolvePluginConfig(current, "python3");
      expect(result.interpreter).toBe("python3");
      expect(result.settings).toBeUndefined();
    });
  });

  describe("getDisplayInterpreter", () => {
    it("returns the configured interpreter", () => {
      const config: PluginConfig = { interpreter: "python3" };
      expect(getDisplayInterpreter(config)).toBe("python3");
    });

    it("returns empty string when config is undefined", () => {
      expect(getDisplayInterpreter(undefined)).toBe("");
    });

    it("returns empty string when interpreter is undefined", () => {
      const config: PluginConfig = { settings: {} };
      expect(getDisplayInterpreter(config)).toBe("");
    });
  });
});
