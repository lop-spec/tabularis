import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  FONT_MAP,
  DEFAULT_FONT_SIZE,
  DEFAULT_FONT_FAMILY,
  FONT_CACHE_KEY,
  OLD_SETTINGS_KEY,
  AVAILABLE_FONTS,
  ROADMAP,
  getFontCSS,
  createFontCSSVariables,
  loadFontCache,
  saveFontCache,
  migrateFromLocalStorage,
  mergeSettings,
  applyFontToDocument,
  getLanguageForI18n,
  type FontCache,
} from '../../src/utils/settings';
import { DEFAULT_SETTINGS } from '../../src/contexts/SettingsContext';
import type { Settings, AppLanguage } from '../../src/contexts/SettingsContext';

describe('settings', () => {
  const localStorageMock = {
    getItem: vi.fn(),
    setItem: vi.fn(),
    removeItem: vi.fn(),
    clear: vi.fn(),
  };

  beforeEach(() => {
    Object.defineProperty(window, 'localStorage', {
      value: localStorageMock,
      writable: true,
    });
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('FONT_MAP', () => {
    it('should contain all predefined fonts', () => {
      expect(FONT_MAP).toHaveProperty('System');
      expect(FONT_MAP).toHaveProperty('Open Sans');
      expect(FONT_MAP).toHaveProperty('Roboto');
      expect(FONT_MAP).toHaveProperty('JetBrains Mono');
      expect(FONT_MAP).toHaveProperty('Hack');
      expect(FONT_MAP).toHaveProperty('Menlo');
      expect(FONT_MAP).toHaveProperty('DejaVu Sans Mono');
    });

    it('should have System as fallback with system-ui stack', () => {
      expect(FONT_MAP['System']).toContain('system-ui');
      expect(FONT_MAP['System']).toContain('-apple-system');
    });

    it('should have monospace fonts for code editors', () => {
      expect(FONT_MAP['JetBrains Mono']).toContain('monospace');
      expect(FONT_MAP['Hack']).toContain('monospace');
    });
  });

  describe('getFontCSS', () => {
    it('should return mapped font for predefined fonts', () => {
      expect(getFontCSS('System')).toBe(FONT_MAP['System']);
      expect(getFontCSS('Roboto')).toBe(FONT_MAP['Roboto']);
    });

    it('should return custom font as-is if not in map', () => {
      expect(getFontCSS('Custom Font')).toBe('Custom Font');
    });

    it('should return System font as fallback for empty string', () => {
      expect(getFontCSS('')).toBe(FONT_MAP['System']);
    });
  });

  describe('createFontCSSVariables', () => {
    it('should create CSS variables with font family and size', () => {
      const result = createFontCSSVariables('Roboto', 16);
      
      expect(result).toEqual({
        '--font-base': FONT_MAP['Roboto'],
        '--font-size-base': '16px',
      });
    });

    it('should handle custom font family', () => {
      const result = createFontCSSVariables('Custom Font', 14);
      
      expect(result['--font-base']).toBe('Custom Font');
    });

    it('should use default size when size is 0', () => {
      const result = createFontCSSVariables('System', 0);
      
      expect(result['--font-size-base']).toBe(`${DEFAULT_FONT_SIZE}px`);
    });
  });

  describe('loadFontCache', () => {
    it('should return null when no cache exists', () => {
      localStorageMock.getItem.mockReturnValue(null);
      
      const result = loadFontCache();
      
      expect(result).toBeNull();
      expect(localStorageMock.getItem).toHaveBeenCalledWith(FONT_CACHE_KEY);
    });

    it('should return parsed cache when exists', () => {
      const cache: FontCache = { fontFamily: 'Roboto', fontSize: 16 };
      localStorageMock.getItem.mockReturnValue(JSON.stringify(cache));
      
      const result = loadFontCache();
      
      expect(result).toEqual(cache);
    });

    it('should return null and warn on parse error', () => {
      localStorageMock.getItem.mockReturnValue('invalid json');
      const consoleSpy = vi.spyOn(console, 'warn').mockImplementation(() => { });
      
      const result = loadFontCache();
      
      expect(result).toBeNull();
      expect(consoleSpy).toHaveBeenCalled();
      consoleSpy.mockRestore();
    });
  });

  describe('saveFontCache', () => {
    it('should save font cache to localStorage', () => {
      saveFontCache('Roboto', 16);
      
      expect(localStorageMock.setItem).toHaveBeenCalledWith(
        FONT_CACHE_KEY,
        JSON.stringify({ fontFamily: 'Roboto', fontSize: 16 })
      );
    });

    it('should warn on localStorage error', () => {
      localStorageMock.setItem.mockImplementation(() => {
        throw new Error('Storage full');
      });
      const consoleSpy = vi.spyOn(console, 'warn').mockImplementation(() => { });
      
      saveFontCache('System', 14);
      
      expect(consoleSpy).toHaveBeenCalled();
      consoleSpy.mockRestore();
    });
  });

  describe('migrateFromLocalStorage', () => {
    it('should not migrate when backend config has data', () => {
      const backendConfig: Partial<Settings> = {
        resultPageSize: 1000,
        language: 'en',
      };
      
      const result = migrateFromLocalStorage(backendConfig);
      
      expect(result.migrated).toBe(false);
      expect(result.settings).toEqual(backendConfig);
    });

    it('should migrate from localStorage when backend is empty', () => {
      const backendConfig: Partial<Settings> = {};
      const oldData = { queryLimit: 200, language: 'it' };
      localStorageMock.getItem.mockReturnValue(JSON.stringify(oldData));
      
      const result = migrateFromLocalStorage(backendConfig);
      
      expect(result.migrated).toBe(true);
      expect(result.settings.resultPageSize).toBe(200);
      expect(result.settings.language).toBe('it');
    });

    it('should use defaults when localStorage data is incomplete', () => {
      const backendConfig: Partial<Settings> = {};
      localStorageMock.getItem.mockReturnValue(JSON.stringify({}));
      
      const result = migrateFromLocalStorage(backendConfig);
      
      expect(result.migrated).toBe(true);
      expect(result.settings.resultPageSize).toBe(500);
      expect(result.settings.language).toBe('auto');
    });

    it('should not migrate when localStorage is empty', () => {
      const backendConfig: Partial<Settings> = {};
      localStorageMock.getItem.mockReturnValue(null);
      
      const result = migrateFromLocalStorage(backendConfig);
      
      expect(result.migrated).toBe(false);
    });

    it('should handle parse errors gracefully', () => {
      const backendConfig: Partial<Settings> = {};
      localStorageMock.getItem.mockReturnValue('invalid');
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => { });
      
      const result = migrateFromLocalStorage(backendConfig);
      
      expect(result.migrated).toBe(false);
      consoleSpy.mockRestore();
    });
  });

  describe('mergeSettings', () => {
    const defaults: Settings = {
      resultPageSize: 500,
      language: 'auto',
      fontFamily: 'System',
      fontSize: 14,
    };

    it('should use defaults when no overrides', () => {
      const result = mergeSettings(defaults, {}, {}, false);
      
      expect(result).toEqual(defaults);
    });

    it('should override defaults with backend config', () => {
      const backendConfig: Partial<Settings> = {
        resultPageSize: 1000,
        language: 'en',
      };
      
      const result = mergeSettings(defaults, backendConfig, {}, false);
      
      expect(result.resultPageSize).toBe(1000);
      expect(result.language).toBe('en');
      expect(result.fontFamily).toBe('System'); // default
    });

    it('should use migrated settings when wasMigrated is true', () => {
      const migratedSettings: Partial<Settings> = {
        resultPageSize: 200,
        language: 'it',
      };
      
      const result = mergeSettings(defaults, {}, migratedSettings, true);
      
      expect(result.resultPageSize).toBe(200);
      expect(result.language).toBe('it');
    });

  });

  describe('applyFontToDocument', () => {
    beforeEach(() => {
      // Mock document
      const styleMock = {
        setProperty: vi.fn(),
      };
      Object.defineProperty(document, 'documentElement', {
        value: { style: styleMock },
        writable: true,
      });
      Object.defineProperty(document, 'body', {
        value: { style: { ...styleMock, fontFamily: '', fontSize: '' } },
        writable: true,
      });
    });

    it('should apply font CSS variables to document', () => {
      applyFontToDocument('Roboto', 16);
      
      expect(document.documentElement.style.setProperty).toHaveBeenCalledWith(
        '--font-base',
        FONT_MAP['Roboto']
      );
      expect(document.documentElement.style.setProperty).toHaveBeenCalledWith(
        '--font-size-base',
        '16px'
      );
    });

    it('should apply font directly to body', () => {
      applyFontToDocument('System', 14);
      
      expect(document.body.style.fontFamily).toBe(FONT_MAP['System']);
      expect(document.body.style.fontSize).toBe('14px');
    });

    it('should handle server-side rendering', () => {
      Object.defineProperty(global, 'document', {
        value: undefined,
        writable: true,
      });
      
      // Should not throw
      expect(() => applyFontToDocument('System', 14)).not.toThrow();
    });
  });

  describe('getLanguageForI18n', () => {
    it('should return undefined for auto language', () => {
      const result = getLanguageForI18n('auto');
      
      expect(result).toBeUndefined();
    });

    it('should return language code for specific language', () => {
      expect(getLanguageForI18n('en')).toBe('en');
      expect(getLanguageForI18n('it')).toBe('it');
    });

    it('should use system language when provided with auto', () => {
      const result = getLanguageForI18n('auto', 'en-US');
      
      expect(result).toBe('en-US');
    });
  });

  describe('AVAILABLE_FONTS', () => {
    it('should contain System as first option', () => {
      expect(AVAILABLE_FONTS[0]).toEqual({
        name: 'System',
        label: 'System Default (Automatic)',
      });
    });

    it('should contain monospace fonts', () => {
      const monospaceFonts = AVAILABLE_FONTS.filter(
        f => f.name.includes('Mono') || f.name === 'Hack'
      );
      expect(monospaceFonts.length).toBeGreaterThanOrEqual(3);
      expect(monospaceFonts.map(f => f.name)).toContain('JetBrains Mono');
      expect(monospaceFonts.map(f => f.name)).toContain('DejaVu Sans Mono');
      expect(monospaceFonts.map(f => f.name)).toContain('Hack');
    });

    it('should have name and label properties for all fonts', () => {
      AVAILABLE_FONTS.forEach(font => {
        expect(font).toHaveProperty('name');
        expect(font).toHaveProperty('label');
        expect(typeof font.name).toBe('string');
        expect(typeof font.label).toBe('string');
        expect(font.name.length).toBeGreaterThan(0);
        expect(font.label.length).toBeGreaterThan(0);
      });
    });

    it('should have unique font names', () => {
      const names = AVAILABLE_FONTS.map(f => f.name);
      const uniqueNames = [...new Set(names)];
      expect(uniqueNames.length).toBe(names.length);
    });
  });

  describe('DEFAULT_SETTINGS editor fields', () => {
    it('should include editor font family defaulting to JetBrains Mono', () => {
      expect(DEFAULT_SETTINGS.editorFontFamily).toBe('JetBrains Mono');
    });

    it('should include editor font size defaulting to 14', () => {
      expect(DEFAULT_SETTINGS.editorFontSize).toBe(14);
    });

    it('should include editor line height defaulting to 1.5', () => {
      expect(DEFAULT_SETTINGS.editorLineHeight).toBe(1.5);
    });

    it('should include editor tab size defaulting to 2', () => {
      expect(DEFAULT_SETTINGS.editorTabSize).toBe(2);
    });

    it('should enable word wrap by default', () => {
      expect(DEFAULT_SETTINGS.editorWordWrap).toBe(true);
    });

    it('should show line numbers by default', () => {
      expect(DEFAULT_SETTINGS.editorShowLineNumbers).toBe(true);
    });

    it('should not set an editor theme override by default', () => {
      expect(DEFAULT_SETTINGS.editorTheme).toBeUndefined();
    });
  });

  describe('mergeSettings with editor fields', () => {
    const defaults: Settings = {
      ...DEFAULT_SETTINGS,
    };

    it('should carry editor settings from backend config', () => {
      const backendConfig: Partial<Settings> = {
        editorFontFamily: 'Hack',
        editorFontSize: 16,
        editorTabSize: 4,
        editorWordWrap: false,
        editorShowLineNumbers: false,
        editorLineHeight: 1.8,
        editorTheme: 'tabularis-light',
      };

      const result = mergeSettings(defaults, backendConfig, {}, false);

      expect(result.editorFontFamily).toBe('Hack');
      expect(result.editorFontSize).toBe(16);
      expect(result.editorTabSize).toBe(4);
      expect(result.editorWordWrap).toBe(false);
      expect(result.editorShowLineNumbers).toBe(false);
      expect(result.editorLineHeight).toBe(1.8);
      expect(result.editorTheme).toBe('tabularis-light');
    });

    it('should fall back to defaults when editor fields are absent from backend', () => {
      const result = mergeSettings(defaults, {}, {}, false);

      expect(result.editorFontFamily).toBe('JetBrains Mono');
      expect(result.editorFontSize).toBe(14);
      expect(result.editorWordWrap).toBe(true);
      expect(result.editorShowLineNumbers).toBe(true);
    });

    it('should treat empty editorTheme string as no override', () => {
      const backendConfig: Partial<Settings> = { editorTheme: '' };
      const result = mergeSettings(defaults, backendConfig, {}, false);
      expect(result.editorTheme).toBe('');
      // Falsy check that the wrapper uses to detect "inherit from app"
      expect(!result.editorTheme).toBe(true);
    });
  });
});
