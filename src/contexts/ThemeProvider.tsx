import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ThemeContext } from './ThemeContext';
import { themeRegistry } from '../themes/themeRegistry';
import { applyThemeToCSS } from '../themes/themeUtils';
import type { Theme, ThemeSettings } from '../types/theme';

const THEME_SETTINGS_KEY = 'tabularis_theme_settings';

const DEFAULT_THEME_SETTINGS: ThemeSettings = {
  activeThemeId: 'tabularis-dark',
  followSystemTheme: false,
  lightThemeId: 'tabularis-light',
  darkThemeId: 'tabularis-dark',
  customThemes: [],
};

export const ThemeProvider = ({ children }: { children: ReactNode }) => {
  const [currentTheme, setCurrentTheme] = useState<Theme>(() => themeRegistry.getDefault());
  const [settings, setSettings] = useState<ThemeSettings>(DEFAULT_THEME_SETTINGS);
  const [customThemes, setCustomThemes] = useState<Theme[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  // Combine all themes
  const allThemes = useMemo(() => {
    const presets = themeRegistry.getAllPresets();
    return [...presets, ...customThemes];
  }, [customThemes]);

  // Load theme settings and custom themes on mount
  useEffect(() => {
    const loadThemes = async () => {
      try {
        // Load settings from localStorage (lightweight, UI-related)
        const savedSettings = localStorage.getItem(THEME_SETTINGS_KEY);
        let parsedSettings = DEFAULT_THEME_SETTINGS;
        
        if (savedSettings) {
          parsedSettings = { ...DEFAULT_THEME_SETTINGS, ...JSON.parse(savedSettings) };
        }

        // Load custom themes from backend
        const loadedCustomThemes = await invoke<Theme[]>('get_all_themes').catch(() => [] as Theme[]);
        setCustomThemes(loadedCustomThemes.filter(t => !t.isPreset));

        // Set initial theme
        const allAvailableThemes = [...themeRegistry.getAllPresets(), ...loadedCustomThemes];
        const initialTheme = allAvailableThemes.find(t => t.id === parsedSettings.activeThemeId);
        
        if (initialTheme) {
          setCurrentTheme(initialTheme);
        }
        
        setSettings(parsedSettings);
      } catch (error) {
        console.error('Failed to load themes:', error);
      } finally {
        setIsLoading(false);
      }
    };

    loadThemes();
  }, []);

  // Apply theme to CSS when currentTheme changes
  useEffect(() => {
    if (currentTheme) {
      applyThemeToCSS(currentTheme);
    }
  }, [currentTheme]);

  // Save settings to localStorage when they change
  useEffect(() => {
    if (!isLoading) {
      localStorage.setItem(THEME_SETTINGS_KEY, JSON.stringify(settings));
    }
  }, [settings, isLoading]);

  const setTheme = useCallback(async (themeId: string) => {
    const theme = allThemes.find(t => t.id === themeId);
    if (!theme) {
      throw new Error(`Theme ${themeId} not found`);
    }
    
    setCurrentTheme(theme);
    setSettings(prev => ({ ...prev, activeThemeId: themeId }));
  }, [allThemes]);

  const createCustomTheme = useCallback(async (baseThemeId: string, name: string): Promise<Theme> => {
    const baseTheme = allThemes.find(t => t.id === baseThemeId);
    if (!baseTheme) {
      throw new Error('Base theme not found');
    }

    const newTheme: Theme = {
      ...baseTheme,
      id: `custom-${Date.now()}`,
      name,
      isPreset: false,
      isReadOnly: false,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    };

    await invoke('save_custom_theme', { theme: newTheme });
    
    setCustomThemes(prev => [...prev, newTheme]);
    setSettings(prev => ({ ...prev, customThemes: [...prev.customThemes, newTheme.id] }));
    
    return newTheme;
  }, [allThemes]);

  const updateCustomTheme = useCallback(async (theme: Theme) => {
    if (theme.isPreset) {
      throw new Error('Cannot modify preset themes');
    }

    const updatedTheme: Theme = {
      ...theme,
      updatedAt: new Date().toISOString(),
    };

    await invoke('save_custom_theme', { theme: updatedTheme });
    
    setCustomThemes(prev => 
      prev.map(t => t.id === updatedTheme.id ? updatedTheme : t)
    );
    
    // Update current theme if it's the one being edited
    if (currentTheme.id === updatedTheme.id) {
      setCurrentTheme(updatedTheme);
    }
  }, [currentTheme.id]);

  const deleteCustomTheme = useCallback(async (themeId: string) => {
    const theme = allThemes.find(t => t.id === themeId);
    if (!theme || theme.isPreset) {
      throw new Error('Cannot delete preset themes');
    }

    await invoke('delete_custom_theme', { themeId });
    
    setCustomThemes(prev => prev.filter(t => t.id !== themeId));
    setSettings(prev => ({
      ...prev,
      customThemes: prev.customThemes.filter(id => id !== themeId),
    }));
    
    // If the deleted theme was active, switch to default
    if (currentTheme.id === themeId) {
      const defaultTheme = themeRegistry.getDefault();
      setCurrentTheme(defaultTheme);
      setSettings(prev => ({ ...prev, activeThemeId: defaultTheme.id }));
    }
  }, [allThemes, currentTheme.id]);

  const duplicateTheme = useCallback(async (themeId: string, newName: string): Promise<Theme> => {
    const baseTheme = allThemes.find(t => t.id === themeId);
    if (!baseTheme) {
      throw new Error('Theme not found');
    }

    const duplicatedTheme: Theme = {
      ...baseTheme,
      id: `custom-${Date.now()}`,
      name: newName,
      isPreset: false,
      isReadOnly: false,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    };

    await invoke('save_custom_theme', { theme: duplicatedTheme });
    
    setCustomThemes(prev => [...prev, duplicatedTheme]);
    setSettings(prev => ({ ...prev, customThemes: [...prev.customThemes, duplicatedTheme.id] }));
    
    return duplicatedTheme;
  }, [allThemes]);

  const importTheme = useCallback(async (themeJson: string): Promise<Theme> => {
    const importedTheme: Theme = JSON.parse(themeJson);
    
    // Ensure it's marked as custom
    const customTheme: Theme = {
      ...importedTheme,
      id: `custom-${Date.now()}`,
      isPreset: false,
      isReadOnly: false,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    };

    await invoke('save_custom_theme', { theme: customTheme });
    
    setCustomThemes(prev => [...prev, customTheme]);
    setSettings(prev => ({ ...prev, customThemes: [...prev.customThemes, customTheme.id] }));
    
    return customTheme;
  }, []);

  const exportTheme = useCallback(async (themeId: string): Promise<string> => {
    const theme = allThemes.find(t => t.id === themeId);
    if (!theme) {
      throw new Error('Theme not found');
    }

    return JSON.stringify(theme, null, 2);
  }, [allThemes]);

  const updateSettings = useCallback(async (newSettings: Partial<ThemeSettings>) => {
    setSettings(prev => ({ ...prev, ...newSettings }));
    
    // If followSystemTheme is enabled, we would need to set up a media query listener
    // This is handled separately in a dedicated effect
  }, []);

  const getMonacoThemeJson = useCallback(async () => {
    // This method is deprecated - Monaco themes are loaded automatically
    // Kept for backwards compatibility
    const monaco = await import('monaco-editor');
    const themeName = currentTheme.monacoTheme.themeName || 'vs-dark';
    return monaco.editor.getTheme(themeName);
  }, [currentTheme]);

  const value = useMemo(() => ({
    currentTheme,
    settings,
    allThemes,
    isLoading,
    setTheme,
    createCustomTheme,
    updateCustomTheme,
    deleteCustomTheme,
    duplicateTheme,
    importTheme,
    exportTheme,
    updateSettings,
    getMonacoThemeJson,
  }), [
    currentTheme,
    settings,
    allThemes,
    isLoading,
    setTheme,
    createCustomTheme,
    updateCustomTheme,
    deleteCustomTheme,
    duplicateTheme,
    importTheme,
    exportTheme,
    updateSettings,
    getMonacoThemeJson,
  ]);

  return (
    <ThemeContext.Provider value={value}>
      {children}
    </ThemeContext.Provider>
  );
};
