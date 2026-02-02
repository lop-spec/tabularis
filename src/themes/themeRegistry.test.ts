import { describe, it, expect } from 'vitest';
import { themeRegistry } from './themeRegistry';
import { tabularisDark } from './presets/tabularisDark';
import { tabularisLight } from './presets/tabularisLight';
import { monokai } from './presets/monokai';
import { oneDarkPro } from './presets/oneDarkPro';
import { nord } from './presets/nord';
import { dracula } from './presets/dracula';
import { githubDark } from './presets/githubDark';
import { solarizedDark } from './presets/solarizedDark';
import { solarizedLight } from './presets/solarizedLight';
import { highContrast } from './presets/highContrast';
import type { Theme } from '../types/theme';

describe('themeRegistry', () => {
  describe('getPreset', () => {
    it('should return tabularis-dark preset', () => {
      const theme = themeRegistry.getPreset('tabularis-dark');
      expect(theme).toBeDefined();
      expect(theme?.id).toBe('tabularis-dark');
      expect(theme?.isPreset).toBe(true);
    });

    it('should return tabularis-light preset', () => {
      const theme = themeRegistry.getPreset('tabularis-light');
      expect(theme).toBeDefined();
      expect(theme?.id).toBe('tabularis-light');
    });

    it('should return undefined for unknown preset', () => {
      const theme = themeRegistry.getPreset('unknown-theme');
      expect(theme).toBeUndefined();
    });

    it('should return monokai preset', () => {
      const theme = themeRegistry.getPreset('monokai');
      expect(theme).toBeDefined();
      expect(theme?.id).toBe('monokai');
    });

    it('should return one-dark-pro preset', () => {
      const theme = themeRegistry.getPreset('one-dark-pro');
      expect(theme).toBeDefined();
      expect(theme?.id).toBe('one-dark-pro');
    });
  });

  describe('getAllPresets', () => {
    it('should return all 10 presets', () => {
      const presets = themeRegistry.getAllPresets();
      expect(presets).toHaveLength(10);
    });

    it('should return only preset themes', () => {
      const presets = themeRegistry.getAllPresets();
      presets.forEach((preset) => {
        expect(preset.isPreset).toBe(true);
      });
    });

    it('should include all expected theme IDs', () => {
      const presets = themeRegistry.getAllPresets();
      const ids = presets.map((p) => p.id);
      expect(ids).toContain('tabularis-dark');
      expect(ids).toContain('tabularis-light');
      expect(ids).toContain('monokai');
      expect(ids).toContain('one-dark-pro');
      expect(ids).toContain('nord');
      expect(ids).toContain('dracula');
      expect(ids).toContain('github-dark');
      expect(ids).toContain('solarized-dark');
      expect(ids).toContain('solarized-light');
      expect(ids).toContain('high-contrast');
    });
  });

  describe('getDefault', () => {
    it('should return tabularis-dark as default', () => {
      const defaultTheme = themeRegistry.getDefault();
      expect(defaultTheme.id).toBe('tabularis-dark');
    });

    it('should return a valid theme object', () => {
      const defaultTheme = themeRegistry.getDefault();
      expect(defaultTheme).toHaveProperty('id');
      expect(defaultTheme).toHaveProperty('name');
      expect(defaultTheme).toHaveProperty('colors');
      expect(defaultTheme).toHaveProperty('monacoTheme');
    });
  });

  describe('isDarkTheme', () => {
    it('should identify dark themes correctly', () => {
      expect(themeRegistry.isDarkTheme(tabularisDark)).toBe(true);
      expect(themeRegistry.isDarkTheme(monokai)).toBe(true);
      expect(themeRegistry.isDarkTheme(dracula)).toBe(true);
      expect(themeRegistry.isDarkTheme(githubDark)).toBe(true);
      expect(themeRegistry.isDarkTheme(nord)).toBe(true);
    });

    it('should identify non-dark themes as false', () => {
      expect(themeRegistry.isDarkTheme(tabularisLight)).toBe(false);
      expect(themeRegistry.isDarkTheme(solarizedLight)).toBe(false);
    });

    it('should handle high contrast theme', () => {
      // High contrast uses 'hc-black' base
      expect(themeRegistry.isDarkTheme(highContrast)).toBe(true);
    });
  });

  describe('isLightTheme', () => {
    it('should identify light themes correctly', () => {
      expect(themeRegistry.isLightTheme(tabularisLight)).toBe(true);
      expect(themeRegistry.isLightTheme(solarizedLight)).toBe(true);
    });

    it('should identify non-light themes as false', () => {
      expect(themeRegistry.isLightTheme(tabularisDark)).toBe(false);
      expect(themeRegistry.isLightTheme(monokai)).toBe(false);
      expect(themeRegistry.isLightTheme(dracula)).toBe(false);
    });
  });

  describe('Theme structure', () => {
    const testThemeStructure = (theme: Theme) => {
      it(`${theme.name} should have valid structure`, () => {
        // Basic properties
        expect(theme).toHaveProperty('id');
        expect(theme).toHaveProperty('name');
        expect(theme).toHaveProperty('isPreset', true);
        expect(theme).toHaveProperty('colors');
        expect(theme).toHaveProperty('typography');
        expect(theme).toHaveProperty('layout');
        expect(theme).toHaveProperty('monacoTheme');

        // Colors structure
        expect(theme.colors).toHaveProperty('bg');
        expect(theme.colors).toHaveProperty('surface');
        expect(theme.colors).toHaveProperty('text');
        expect(theme.colors).toHaveProperty('accent');
        expect(theme.colors).toHaveProperty('border');
        expect(theme.colors).toHaveProperty('semantic');

        // Monaco theme structure
        expect(theme.monacoTheme).toHaveProperty('base');
        expect(theme.monacoTheme).toHaveProperty('inherit');
        expect(['vs', 'vs-dark', 'hc-black']).toContain(theme.monacoTheme.base);
      });
    };

    // Test all presets
    themeRegistry.getAllPresets().forEach(testThemeStructure);
  });

  describe('Theme uniqueness', () => {
    it('should have unique IDs', () => {
      const presets = themeRegistry.getAllPresets();
      const ids = presets.map((p) => p.id);
      const uniqueIds = new Set(ids);
      expect(uniqueIds.size).toBe(ids.length);
    });

    it('should have unique names', () => {
      const presets = themeRegistry.getAllPresets();
      const names = presets.map((p) => p.name);
      const uniqueNames = new Set(names);
      expect(uniqueNames.size).toBe(names.length);
    });
  });
});
