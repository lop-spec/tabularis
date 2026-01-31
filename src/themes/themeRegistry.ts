import type { Theme } from '../types/theme';
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

class ThemeRegistry {
  private presets: Map<string, Theme> = new Map();

  constructor() {
    this.registerPreset(tabularisDark);
    this.registerPreset(tabularisLight);
    this.registerPreset(monokai);
    this.registerPreset(oneDarkPro);
    this.registerPreset(nord);
    this.registerPreset(dracula);
    this.registerPreset(githubDark);
    this.registerPreset(solarizedDark);
    this.registerPreset(solarizedLight);
    this.registerPreset(highContrast);
  }

  private registerPreset(theme: Theme): void {
    this.presets.set(theme.id, theme);
  }

  getPreset(id: string): Theme | undefined {
    return this.presets.get(id);
  }

  getAllPresets(): Theme[] {
    return Array.from(this.presets.values());
  }

  getDefault(): Theme {
    return this.presets.get('tabularis-dark') ?? tabularisDark;
  }

  isDarkTheme(theme: Theme): boolean {
    return theme.monacoTheme.base === 'vs-dark' || theme.monacoTheme.base === 'hc-black';
  }

  isLightTheme(theme: Theme): boolean {
    return theme.monacoTheme.base === 'vs';
  }
}

export const themeRegistry = new ThemeRegistry();
