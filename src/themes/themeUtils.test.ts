import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  loadMonacoTheme,
  applyThemeToCSS,
  generateMonacoTheme,
} from './themeUtils';
import type { Theme } from '../types/theme';
import * as monaco from 'monaco-editor';

// Mock monaco-editor
vi.mock('monaco-editor', () => ({
  editor: {
    defineTheme: vi.fn(),
    setTheme: vi.fn(),
  },
}));

describe('themeUtils', () => {
  const mockTheme: Theme = {
    id: 'test-theme',
    name: 'Test Theme',
    isPreset: false,
    isReadOnly: false,
    colors: {
      bg: {
        base: '#1a1a1a',
        elevated: '#2d2d2d',
        overlay: '#3d3d3d',
        input: '#2a2a2a',
        tooltip: '#333333',
      },
      surface: {
        primary: '#2d2d2d',
        secondary: '#3d3d3d',
        tertiary: '#4d4d4d',
        hover: '#3a3a3a',
        active: '#4a4a4a',
        disabled: '#5d5d5d',
      },
      text: {
        primary: '#ffffff',
        secondary: '#b0b0b0',
        muted: '#808080',
        disabled: '#606060',
        accent: '#3b82f6',
        inverse: '#000000',
      },
      accent: {
        primary: '#3b82f6',
        secondary: '#60a5fa',
        success: '#22c55e',
        warning: '#f59e0b',
        error: '#ef4444',
        info: '#3b82f6',
      },
      border: {
        subtle: '#2d2d2d',
        default: '#3d3d3d',
        strong: '#4d4d4d',
        focus: '#3b82f6',
      },
      semantic: {
        string: '#22c55e',
        number: '#f59e0b',
        boolean: '#3b82f6',
        date: '#a855f7',
        null: '#808080',
        primaryKey: '#f59e0b',
        foreignKey: '#22c55e',
        index: '#3b82f6',
        connectionActive: '#22c55e',
        connectionInactive: '#ef4444',
        modified: '#f59e0b',
        deleted: '#ef4444',
        new: '#22c55e',
      },
    },
    typography: {
      fontFamily: {
        base: 'system-ui',
        mono: 'monospace',
      },
      fontSize: {
        xs: '12px',
        sm: '14px',
        base: '16px',
        lg: '18px',
        xl: '20px',
      },
    },
    layout: {
      borderRadius: {
        sm: '4px',
        base: '6px',
        lg: '8px',
        xl: '12px',
      },
      spacing: {
        xs: '4px',
        sm: '8px',
        base: '16px',
        lg: '24px',
        xl: '32px',
      },
    },
    monacoTheme: {
      base: 'vs-dark',
      inherit: true,
      rules: [],
      colors: {},
    },
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    // Reset document.documentElement styles
    document.documentElement.style.cssText = '';
  });

  describe('loadMonacoTheme', () => {
    it('should define and set theme when no themeName provided', () => {
      loadMonacoTheme(mockTheme);
      expect(monaco.editor.defineTheme).toHaveBeenCalledWith(
        'test-theme',
        expect.any(Object)
      );
      expect(monaco.editor.setTheme).toHaveBeenCalledWith('test-theme');
    });

    it('should use predefined Monaco theme when themeName exists', () => {
      const themeWithName = {
        ...mockTheme,
        id: 'monokai-test', // Use a different ID to avoid cache
        monacoTheme: {
          ...mockTheme.monacoTheme,
          themeName: 'Monokai',
        },
      };
      loadMonacoTheme(themeWithName);
      expect(monaco.editor.defineTheme).toHaveBeenCalledWith(
        'monokai-test',
        expect.any(Object)
      );
      expect(monaco.editor.setTheme).toHaveBeenCalledWith('monokai-test');
    });

    it('should fallback to base theme on error', () => {
      const errorTheme = {
        ...mockTheme,
        id: 'error-theme', // Use a different ID
      };
      vi.mocked(monaco.editor.defineTheme).mockImplementationOnce(() => {
        throw new Error('Theme error');
      });
      loadMonacoTheme(errorTheme);
      expect(monaco.editor.setTheme).toHaveBeenCalledWith('vs-dark');
    });

    it('should accept custom monaco instance', () => {
      const customMonaco = {
        editor: {
          defineTheme: vi.fn(),
          setTheme: vi.fn(),
        },
      } as unknown as typeof monaco;
      
      const customTheme = {
        ...mockTheme,
        id: 'custom-instance-theme', // Use a different ID
      };
      
      loadMonacoTheme(customTheme, customMonaco);
      expect(customMonaco.editor.defineTheme).toHaveBeenCalled();
      expect(customMonaco.editor.setTheme).toHaveBeenCalledWith('custom-instance-theme');
    });
  });

  describe('applyThemeToCSS', () => {
    it('should set background CSS variables', () => {
      applyThemeToCSS(mockTheme);
      const root = document.documentElement;
      expect(root.style.getPropertyValue('--bg-base')).toBe('#1a1a1a');
      expect(root.style.getPropertyValue('--bg-elevated')).toBe('#2d2d2d');
      expect(root.style.getPropertyValue('--bg-overlay')).toBe('#3d3d3d');
    });

    it('should set surface CSS variables', () => {
      applyThemeToCSS(mockTheme);
      const root = document.documentElement;
      expect(root.style.getPropertyValue('--surface-primary')).toBe('#2d2d2d');
      expect(root.style.getPropertyValue('--surface-hover')).toBe('#3a3a3a');
    });

    it('should set text CSS variables', () => {
      applyThemeToCSS(mockTheme);
      const root = document.documentElement;
      expect(root.style.getPropertyValue('--text-primary')).toBe('#ffffff');
      expect(root.style.getPropertyValue('--text-secondary')).toBe('#b0b0b0');
      expect(root.style.getPropertyValue('--text-muted')).toBe('#808080');
    });

    it('should set accent CSS variables', () => {
      applyThemeToCSS(mockTheme);
      const root = document.documentElement;
      expect(root.style.getPropertyValue('--accent-primary')).toBe('#3b82f6');
      expect(root.style.getPropertyValue('--accent-success')).toBe('#22c55e');
      expect(root.style.getPropertyValue('--accent-error')).toBe('#ef4444');
    });

    it('should set border CSS variables', () => {
      applyThemeToCSS(mockTheme);
      const root = document.documentElement;
      expect(root.style.getPropertyValue('--border-default')).toBe('#3d3d3d');
      expect(root.style.getPropertyValue('--border-focus')).toBe('#3b82f6');
    });

    it('should set semantic CSS variables', () => {
      applyThemeToCSS(mockTheme);
      const root = document.documentElement;
      expect(root.style.getPropertyValue('--semantic-string')).toBe('#22c55e');
      expect(root.style.getPropertyValue('--semantic-pk')).toBe('#f59e0b');
      expect(root.style.getPropertyValue('--semantic-fk')).toBe('#22c55e');
    });

    it('should set typography CSS variables', () => {
      applyThemeToCSS(mockTheme);
      const root = document.documentElement;
      expect(root.style.getPropertyValue('--font-base')).toBe('system-ui');
      expect(root.style.getPropertyValue('--font-mono')).toBe('monospace');
    });

    it('should set layout CSS variables', () => {
      applyThemeToCSS(mockTheme);
      const root = document.documentElement;
      expect(root.style.getPropertyValue('--radius-sm')).toBe('4px');
      expect(root.style.getPropertyValue('--radius-base')).toBe('6px');
    });

    it('should set color-scheme based on monaco base', () => {
      applyThemeToCSS(mockTheme);
      const root = document.documentElement;
      expect(root.style.getPropertyValue('color-scheme')).toBe('dark');
    });

    it('should set light color-scheme for light themes', () => {
      const lightTheme = {
        ...mockTheme,
        monacoTheme: { ...mockTheme.monacoTheme, base: 'vs' as const },
      };
      applyThemeToCSS(lightTheme);
      const root = document.documentElement;
      expect(root.style.getPropertyValue('color-scheme')).toBe('light');
    });
  });

  describe('generateMonacoTheme', () => {
    it('should generate theme with correct base', () => {
      const result = generateMonacoTheme(mockTheme);
      expect(result.base).toBe('vs-dark');
    });

    it('should generate theme with inherit flag', () => {
      const result = generateMonacoTheme(mockTheme);
      expect(result.inherit).toBe(true);
    });

    it('should include editor background color', () => {
      const result = generateMonacoTheme(mockTheme);
      expect(result.colors?.['editor.background']).toBe('#1a1a1a');
    });

    it('should include editor foreground color', () => {
      const result = generateMonacoTheme(mockTheme);
      expect(result.colors?.['editor.foreground']).toBe('#ffffff');
    });

    it('should include cursor color', () => {
      const result = generateMonacoTheme(mockTheme);
      expect(result.colors?.['editorCursor.foreground']).toBe('#3b82f6');
    });

    it('should include selection colors', () => {
      const result = generateMonacoTheme(mockTheme);
      expect(result.colors?.['editor.selectionBackground']).toBe('#4a4a4a');
    });

    it('should include line number colors', () => {
      const result = generateMonacoTheme(mockTheme);
      expect(result.colors?.['editorLineNumber.foreground']).toBe('#808080');
      expect(result.colors?.['editorLineNumber.activeForeground']).toBe('#b0b0b0');
    });

    it('should include button colors', () => {
      const result = generateMonacoTheme(mockTheme);
      expect(result.colors?.['button.background']).toBe('#3b82f6');
      expect(result.colors?.['button.foreground']).toBe('#000000');
    });

    it('should include suggest widget colors', () => {
      const result = generateMonacoTheme(mockTheme);
      expect(result.colors?.['editorSuggestWidget.background']).toBe('#2d2d2d');
      expect(result.colors?.['editorSuggestWidget.border']).toBe('#3d3d3d');
    });

    it('should merge base colors from theme', () => {
      const themeWithBaseColors = {
        ...mockTheme,
        monacoTheme: {
          ...mockTheme.monacoTheme,
          colors: { 'custom.color': '#ff0000' },
        },
      };
      const result = generateMonacoTheme(themeWithBaseColors);
      expect(result.colors?.['custom.color']).toBe('#ff0000');
    });
  });
});
