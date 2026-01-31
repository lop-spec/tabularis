# Documento Tecnico: Sistema di Temi Tabularis

## 1. Overview

Sistema di temi completo e flessibile per Tabularis che permette:
- **Temi UI completamente personalizzabili** (palette colori, spaziature, tipografia)
- **Syntax highlighting dinamico** (Monaco Editor themes)
- **Icona taskbar personalizzata** (cambio dinamico icona app Tauri)
- **Preset popolari** (Monokai, One Dark, Nord, Dracula, etc.)
- **Import/Export temi** (condivisione con la community)

---

## 2. Architecture Overview

### 2.1 Stack Tecnologico
- **Frontend**: React + Tailwind CSS v4 + CSS Custom Properties
- **Backend**: Tauri (Rust) + File System API
- **Editor**: Monaco Editor (temi JSON personalizzati)
- **Persistenza**: `config.json` (tema attivo) + `themes/` directory (temi custom)

### 2.2 Componenti Principali

```
themes/
├── ThemeContext.tsx         # Gestione stato tema globale
├── ThemeProvider.tsx        # Provider + applicazione CSS vars
├── themeRegistry.ts         # Registro preset temi
├── themeUtils.ts            # Utility conversione colori
├── monacoThemes.ts          # Generatori temi Monaco
└── types.ts                 # TypeScript interfaces

Backend (Rust):
├── theme_commands.rs        # Comandi Tauri per temi
├── theme_persistence.rs     # Salva/carica temi da disco
└── theme_assets.rs          # Gestione icona taskbar

UI:
├── ThemeSettingsPage.tsx    # Pagina gestione temi
├── ThemeEditor.tsx          # Editor tema completo
├── ThemeSelector.tsx        # Selezione rapida tema
└── ThemePreview.tsx         # Preview componenti
```

---

## 3. Struttura Dati

### 3.1 Interfaccia Tema

```typescript
// src/types/theme.ts

export interface Theme {
  id: string;                    // UUID o slug preset
  name: string;                  // Nome visualizzato
  author?: string;               // Autore tema
  version?: string;              // Versione tema
  isPreset: boolean;             // true = built-in, false = custom
  isReadOnly: boolean;           // Non modificabile (preset)
  createdAt?: string;            // ISO date
  updatedAt?: string;            // ISO date
  
  // Palette colori UI
  colors: {
    // Background layers
    bg: {
      base: string;              // Sfondo principale (editor, sidebar)
      elevated: string;          // Elementi sopra (cards, panels)
      overlay: string;           // Overlay, modali
      input: string;             // Input fields
      tooltip: string;           // Tooltips
    };
    
    // Surface/UI elements
    surface: {
      primary: string;           // Sidebar, panels
      secondary: string;         // Subtle backgrounds
      tertiary: string;          // Hover states
      hover: string;             // Hover generico
      active: string;            // Active/selected
      disabled: string;          // Disabled state
    };
    
    // Text
    text: {
      primary: string;           // Testo principale
      secondary: string;         // Testo secondario
      muted: string;             // Testo attenuato
      disabled: string;          // Testo disabilitato
      accent: string;            // Testo evidenziato
      inverse: string;           // Testo su sfondi scuri (se necessario)
    };
    
    // Accents/Status
    accent: {
      primary: string;           // Colore brand primario (blu)
      secondary: string;         // Colore secondario
      success: string;           // Successo, verde
      warning: string;           // Warning, giallo/arancio
      error: string;             // Errore, rosso
      info: string;              // Info, blu chiaro
    };
    
    // Borders
    border: {
      subtle: string;            // Bordi molto leggeri
      default: string;           // Bordi standard
      strong: string;            // Bordi evidenti
      focus: string;             // Focus ring
    };
    
    // Semantic colors (specifici per database/UI)
    semantic: {
      // Tipi di dati SQL
      string: string;            // VARCHAR, TEXT (rosso/arancio)
      number: string;            // INT, FLOAT (blu)
      boolean: string;           // BOOL (viola)
      date: string;              // DATE, DATETIME (verde)
      null: string;              // NULL (grigio/rosso)
      primaryKey: string;        // PK indicator
      foreignKey: string;        // FK indicator
      index: string;             // Index indicator
      
      // UI specific
      connectionActive: string;  // Connessione attiva
      connectionInactive: string;
      modified: string;          // Dati modificati (pending)
      deleted: string;           // Righe marcate per deletion
      new: string;               // Nuove righe
    };
  };
  
  // Typography
  typography: {
    fontFamily: {
      base: string;              // Font UI
      mono: string;              // Font monospace (editor, dati)
    };
    fontSize: {
      xs: string;
      sm: string;
      base: string;
      lg: string;
      xl: string;
    };
  };
  
  // Spacing & Layout
  layout: {
    borderRadius: {
      sm: string;
      base: string;
      lg: string;
      xl: string;
    };
    spacing: {
      xs: string;
      sm: string;
      base: string;
      lg: string;
      xl: string;
    };
  };
  
  // Monaco Editor Theme
  monacoTheme: MonacoThemeDefinition;
  
  // Taskbar Icon (optional)
  taskbarIcon?: {
    type: 'tint' | 'custom';
    color?: string;              // Tint color se type='tint'
    iconPath?: string;           // Path icona custom se type='custom'
  };
}

// Monaco Editor Theme JSON structure
export interface MonacoThemeDefinition {
  base: 'vs' | 'vs-dark' | 'hc-black';  // Base theme
  inherit: boolean;
  rules: Array<{
    token: string;
    foreground?: string;
    background?: string;
    fontStyle?: string;
  }>;
  colors: Record<string, string>;  // UI colors per Monaco
}
```

### 3.2 Active Theme Configuration

```typescript
// src/types/settings.ts

export interface ThemeSettings {
  activeThemeId: string;         // ID tema attivo
  followSystemTheme: boolean;    // Auto-switch light/dark
  lightThemeId: string;          // Tema light quando followSystem=true
  darkThemeId: string;           // Tema dark quando followSystem=true
  customThemes: string[];        // Lista ID temi custom creati dall'utente
}
```

---

## 4. Preset Themes (Built-in)

### 4.1 Lista Preset

| Nome | ID | Tipo | Monaco Base | Descrizione |
|------|-----|------|-------------|-------------|
| **Tabularis Dark** | `tabularis-dark` | Dark | vs-dark | Tema default attuale |
| **Tabularis Light** | `tabularis-light` | Light | vs | Versione chiara |
| **Monokai** | `monokai` | Dark | vs-dark | Tema popolare coding |
| **One Dark Pro** | `one-dark-pro` | Dark | vs-dark | Tema Atom/VS Code |
| **Nord** | `nord` | Dark | vs-dark | Tema polare blu |
| **Dracula** | `dracula` | Dark | vs-dark | Tema purple/pink |
| **GitHub Dark** | `github-dark` | Dark | vs-dark | Tema GitHub |
| **Solarized Dark** | `solarized-dark` | Dark | vs-dark | Tema classico |
| **Solarized Light** | `solarized-light` | Light | vs | Versione chiara |
| **High Contrast** | `high-contrast` | Dark | hc-black | Accessibilità |

### 4.2 Esempio: One Dark Pro

```typescript
// Preset One Dark Pro (VS Code default)
const oneDarkPro: Theme = {
  id: 'one-dark-pro',
  name: 'One Dark Pro',
  isPreset: true,
  isReadOnly: true,
  colors: {
    bg: {
      base: '#282c34',
      elevated: '#21252b',
      overlay: '#2c313a',
      input: '#1e2227',
      tooltip: '#21252b'
    },
    surface: {
      primary: '#21252b',
      secondary: '#2c313a',
      tertiary: '#373d48',
      hover: '#2c313a',
      active: '#3e4451',
      disabled: '#21252b'
    },
    text: {
      primary: '#abb2bf',
      secondary: '#828997',
      muted: '#5c6370',
      disabled: '#4b5263',
      accent: '#61afef',
      inverse: '#282c34'
    },
    accent: {
      primary: '#61afef',
      secondary: '#c678dd',
      success: '#98c379',
      warning: '#e5c07b',
      error: '#e06c75',
      info: '#56b6c2'
    },
    border: {
      subtle: '#21252b',
      default: '#3e4451',
      strong: '#5c6370',
      focus: '#61afef'
    },
    semantic: {
      string: '#e06c75',
      number: '#d19a66',
      boolean: '#c678dd',
      date: '#98c379',
      null: '#5c6370',
      primaryKey: '#e5c07b',
      foreignKey: '#56b6c2',
      index: '#c678dd',
      connectionActive: '#98c379',
      connectionInactive: '#5c6370',
      modified: '#61afef',
      deleted: '#e06c75',
      new: '#98c379'
    }
  },
  monacoTheme: {
    base: 'vs-dark',
    inherit: true,
    rules: [
      { token: 'comment', foreground: '5C6370', fontStyle: 'italic' },
      { token: 'keyword', foreground: 'C678DD' },
      { token: 'string', foreground: '98C379' },
      { token: 'number', foreground: 'D19A66' },
      { token: 'regexp', foreground: '98C379' },
      { token: 'operator', foreground: '56B6C2' },
      { token: 'namespace', foreground: 'E06C75' },
      { token: 'type', foreground: 'E5C07B' },
      { token: 'struct', foreground: 'E5C07B' },
      { token: 'class', foreground: 'E5C07B' },
      { token: 'interface', foreground: 'E5C07B' },
      { token: 'enum', foreground: 'E5C07B' },
      { token: 'typeParameter', foreground: 'E5C07B' },
      { token: 'function', foreground: '61AFEF' },
      { token: 'member', foreground: '61AFEF' },
      { token: 'macro', foreground: '61AFEF' },
      { token: 'variable', foreground: 'E06C75' },
      { token: 'parameter', foreground: 'ABB2BF' },
      { token: 'property', foreground: 'E06C75' },
      { token: 'label', foreground: 'C678DD' }
    ],
    colors: {
      'editor.background': '#282C34',
      'editor.foreground': '#ABB2BF',
      'editorLineNumber.foreground': '#4B5263',
      'editorLineNumber.activeForeground': '#ABB2BF',
      'editor.selectionBackground': '#3E4451',
      'editor.selectionHighlightBackground': '#3E4451',
      'editor.wordHighlightBackground': '#3E4451',
      'editor.wordHighlightStrongBackground': '#3E4451',
      'editor.findMatchBackground': '#5C6370',
      'editor.findMatchHighlightBackground': '#5C6370',
      'editor.findRangeHighlightBackground': '#3E4451',
      'editor.hoverHighlightBackground': '#3E4451',
      'editorCursor.foreground': '#528BFF',
      'editorIndentGuide.background': '#3B4048',
      'editorIndentGuide.activeBackground': '#5C6370',
      'editorLineHighlightBackground': '#2C313A',
      'editorLink.activeForeground': '#61AFEF'
    }
  }
};
```

---

## 5. Backend Implementation (Rust)

### 5.1 Comandi Tauri

```rust
// src-tauri/src/theme_commands.rs

#[tauri::command]
pub fn get_all_themes(app: AppHandle) -> Result<Vec<Theme>, String> {
    // Ritorna tutti i temi: preset + custom
}

#[tauri::command]
pub fn get_theme(app: AppHandle, theme_id: String) -> Result<Theme, String> {
    // Carica singolo tema
}

#[tauri::command]
pub fn save_custom_theme(app: AppHandle, theme: Theme) -> Result<(), String> {
    // Salva tema custom in themes/{theme_id}.json
}

#[tauri::command]
pub fn delete_custom_theme(app: AppHandle, theme_id: String) -> Result<(), String> {
    // Cancella tema custom (solo se !isPreset)
}

#[tauri::command]
pub fn import_theme(app: AppHandle, theme_json: String) -> Result<Theme, String> {
    // Importa tema da JSON string
}

#[tauri::command]
pub fn export_theme(app: AppHandle, theme_id: String) -> Result<String, String> {
    // Esporta tema come JSON string
}

#[tauri::command]
pub fn set_taskbar_icon(app: AppHandle, icon_config: TaskbarIconConfig) -> Result<(), String> {
    // Cambia icona taskbar (Windows: .ico, macOS: .icns, Linux: .png)
}
```

### 5.2 Persistenza

```rust
// src-tauri/src/theme_persistence.rs

const THEMES_DIR: &str = "themes";
const ACTIVE_THEME_FILE: &str = "active_theme.json";

pub fn get_themes_dir(app: &AppHandle) -> PathBuf {
    let config_dir = app.path().app_config_dir().unwrap();
    let themes_dir = config_dir.join(THEMES_DIR);
    if !themes_dir.exists() {
        fs::create_dir_all(&themes_dir).unwrap();
    }
    themes_dir
}

pub fn load_custom_themes(app: &AppHandle) -> Vec<Theme> {
    let themes_dir = get_themes_dir(app);
    let mut themes = Vec::new();
    
    if let Ok(entries) = fs::read_dir(&themes_dir) {
        for entry in entries.flatten() {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                if let Ok(theme) = serde_json::from_str::<Theme>(&content) {
                    themes.push(theme);
                }
            }
        }
    }
    
    themes
}

pub fn save_custom_theme_to_disk(app: &AppHandle, theme: &Theme) -> Result<(), String> {
    let themes_dir = get_themes_dir(app);
    let theme_path = themes_dir.join(format!("{}.json", theme.id));
    let content = serde_json::to_string_pretty(theme).map_err(|e| e.to_string())?;
    fs::write(theme_path, content).map_err(|e| e.to_string())
}
```

### 5.3 Taskbar Icon Management

```rust
// src-tauri/src/theme_assets.rs

use tauri::{AppHandle, Manager};
use tauri::window::WindowBuilder;

#[derive(Serialize, Deserialize)]
pub struct TaskbarIconConfig {
    pub theme_id: String,
    pub color: Option<String>,  // Hex color for tinting
}

pub fn update_taskbar_icon(app: &AppHandle, config: &TaskbarIconConfig) -> Result<(), String> {
    // Nota: Tauri v2 non supporta cambio icona runtime nativamente su tutte le piattaforme
    // Soluzioni alternative:
    
    // macOS: NSApp.applicationIconImage (richiede Objective-C binding)
    // Windows: window.set_icon() + taskbar icon update
    // Linux: window.set_icon()
    
    // Per ora, generiamo icona dinamicamente:
    let icon_path = generate_themed_icon(app, config)?;
    
    // Aggiorna icona finestra principale
    if let Some(window) = app.get_webview_window("main") {
        window.set_icon(icon_path).map_err(|e| e.to_string())?;
    }
    
    Ok(())
}

fn generate_themed_icon(app: &AppHandle, config: &TaskbarIconConfig) -> Result<PathBuf, String> {
    // Genera icona SVG con colore tema, poi converti in formato piattaforma
    let themes_dir = get_themes_dir(app);
    let icon_path = themes_dir.join(format!("icon_{}.png", config.theme_id));
    
    // Usa resvg o simile per renderizzare SVG → PNG
    // Template SVG con colore dinamico
    
    Ok(icon_path)
}
```

---

## 6. Frontend Implementation

### 6.1 ThemeContext & Provider

```typescript
// src/contexts/ThemeContext.tsx

import { createContext, useContext, useEffect, useState, ReactNode } from 'react';
import { Theme, ThemeSettings } from '../types/theme';
import { themeRegistry } from '../themes/themeRegistry';
import { applyThemeToCSS, generateMonacoTheme } from '../themes/themeUtils';

interface ThemeContextType {
  currentTheme: Theme;
  settings: ThemeSettings;
  allThemes: Theme[];
  isLoading: boolean;
  
  // Actions
  setTheme: (themeId: string) => Promise<void>;
  createCustomTheme: (baseThemeId: string, name: string) => Promise<Theme>;
  updateCustomTheme: (theme: Theme) => Promise<void>;
  deleteCustomTheme: (themeId: string) => Promise<void>;
  duplicateTheme: (themeId: string, newName: string) => Promise<Theme>;
  importTheme: (themeJson: string) => Promise<Theme>;
  exportTheme: (themeId: string) => Promise<string>;
  updateSettings: (settings: Partial<ThemeSettings>) => Promise<void>;
  
  // Monaco integration
  getMonacoThemeJson: () => object;
}

const ThemeContext = createContext<ThemeContextType | undefined>(undefined);

export const ThemeProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  const [currentTheme, setCurrentTheme] = useState<Theme>(themeRegistry.getDefault());
  const [settings, setSettings] = useState<ThemeSettings>({
    activeThemeId: 'tabularis-dark',
    followSystemTheme: false,
    lightThemeId: 'tabularis-light',
    darkThemeId: 'tabularis-dark',
    customThemes: []
  });
  const [allThemes, setAllThemes] = useState<Theme[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  // Load themes on mount
  useEffect(() => {
    loadThemes();
  }, []);

  // Apply theme when changed
  useEffect(() => {
    applyThemeToCSS(currentTheme);
    
    // Update Monaco theme if editor exists
    if (window.monaco) {
      const monacoTheme = generateMonacoTheme(currentTheme);
      window.monaco.editor.defineTheme(currentTheme.id, monacoTheme);
      window.monaco.editor.setTheme(currentTheme.id);
    }
    
    // Update taskbar icon
    updateTaskbarIcon(currentTheme);
  }, [currentTheme]);

  const loadThemes = async () => {
    try {
      // Load presets
      const presets = themeRegistry.getAllPresets();
      
      // Load custom themes from backend
      const customThemes = await invoke<Theme[]>('get_all_themes');
      
      setAllThemes([...presets, ...customThemes]);
      
      // Load active theme from settings
      const activeTheme = [...presets, ...customThemes].find(t => t.id === settings.activeThemeId);
      if (activeTheme) {
        setCurrentTheme(activeTheme);
      }
    } finally {
      setIsLoading(false);
    }
  };

  const setTheme = async (themeId: string) => {
    const theme = allThemes.find(t => t.id === themeId);
    if (!theme) throw new Error(`Theme ${themeId} not found`);
    
    setCurrentTheme(theme);
    await updateSettings({ activeThemeId: themeId });
  };

  const createCustomTheme = async (baseThemeId: string, name: string): Promise<Theme> => {
    const baseTheme = allThemes.find(t => t.id === baseThemeId);
    if (!baseTheme) throw new Error('Base theme not found');
    
    const newTheme: Theme = {
      ...baseTheme,
      id: `custom-${Date.now()}`,
      name,
      isPreset: false,
      isReadOnly: false,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString()
    };
    
    await invoke('save_custom_theme', { theme: newTheme });
    await loadThemes();
    
    return newTheme;
  };

  // ... altri metodi

  const value: ThemeContextType = {
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
    getMonacoThemeJson: () => generateMonacoTheme(currentTheme)
  };

  return (
    <ThemeContext.Provider value={value}>
      {children}
    </ThemeContext.Provider>
  );
};

export const useTheme = () => {
  const context = useContext(ThemeContext);
  if (!context) throw new Error('useTheme must be used within ThemeProvider');
  return context;
};
```

### 6.2 Theme Utils (CSS Variables)

```typescript
// src/themes/themeUtils.ts

import { Theme, MonacoThemeDefinition } from '../types/theme';

export function applyThemeToCSS(theme: Theme): void {
  const root = document.documentElement;
  
  // Background
  root.style.setProperty('--bg-base', theme.colors.bg.base);
  root.style.setProperty('--bg-elevated', theme.colors.bg.elevated);
  root.style.setProperty('--bg-overlay', theme.colors.bg.overlay);
  root.style.setProperty('--bg-input', theme.colors.bg.input);
  root.style.setProperty('--bg-tooltip', theme.colors.bg.tooltip);
  
  // Surface
  root.style.setProperty('--surface-primary', theme.colors.surface.primary);
  root.style.setProperty('--surface-secondary', theme.colors.surface.secondary);
  root.style.setProperty('--surface-tertiary', theme.colors.surface.tertiary);
  root.style.setProperty('--surface-hover', theme.colors.surface.hover);
  root.style.setProperty('--surface-active', theme.colors.surface.active);
  root.style.setProperty('--surface-disabled', theme.colors.surface.disabled);
  
  // Text
  root.style.setProperty('--text-primary', theme.colors.text.primary);
  root.style.setProperty('--text-secondary', theme.colors.text.secondary);
  root.style.setProperty('--text-muted', theme.colors.text.muted);
  root.style.setProperty('--text-disabled', theme.colors.text.disabled);
  root.style.setProperty('--text-accent', theme.colors.text.accent);
  root.style.setProperty('--text-inverse', theme.colors.text.inverse);
  
  // Accent
  root.style.setProperty('--accent-primary', theme.colors.accent.primary);
  root.style.setProperty('--accent-secondary', theme.colors.accent.secondary);
  root.style.setProperty('--accent-success', theme.colors.accent.success);
  root.style.setProperty('--accent-warning', theme.colors.accent.warning);
  root.style.setProperty('--accent-error', theme.colors.accent.error);
  root.style.setProperty('--accent-info', theme.colors.accent.info);
  
  // Border
  root.style.setProperty('--border-subtle', theme.colors.border.subtle);
  root.style.setProperty('--border-default', theme.colors.border.default);
  root.style.setProperty('--border-strong', theme.colors.border.strong);
  root.style.setProperty('--border-focus', theme.colors.border.focus);
  
  // Semantic
  root.style.setProperty('--semantic-string', theme.colors.semantic.string);
  root.style.setProperty('--semantic-number', theme.colors.semantic.number);
  root.style.setProperty('--semantic-boolean', theme.colors.semantic.boolean);
  root.style.setProperty('--semantic-date', theme.colors.semantic.date);
  root.style.setProperty('--semantic-null', theme.colors.semantic.null);
  root.style.setProperty('--semantic-pk', theme.colors.semantic.primaryKey);
  root.style.setProperty('--semantic-fk', theme.colors.semantic.foreignKey);
  root.style.setProperty('--semantic-index', theme.colors.semantic.index);
  root.style.setProperty('--semantic-modified', theme.colors.semantic.modified);
  root.style.setProperty('--semantic-deleted', theme.colors.semantic.deleted);
  root.style.setProperty('--semantic-new', theme.colors.semantic.new);
  
  // Typography
  root.style.setProperty('--font-base', theme.typography.fontFamily.base);
  root.style.setProperty('--font-mono', theme.typography.fontFamily.mono);
  
  // Layout
  root.style.setProperty('--radius-sm', theme.layout.borderRadius.sm);
  root.style.setProperty('--radius-base', theme.layout.borderRadius.base);
  root.style.setProperty('--radius-lg', theme.layout.borderRadius.lg);
}

export function generateMonacoTheme(theme: Theme): MonacoThemeDefinition {
  return {
    base: theme.monacoTheme.base,
    inherit: theme.monacoTheme.inherit,
    rules: theme.monacoTheme.rules,
    colors: {
      ...theme.monacoTheme.colors,
      // Override con colori tema attuale
      'editor.background': theme.colors.bg.base,
      'editor.foreground': theme.colors.text.primary,
      'editorLineNumber.foreground': theme.colors.text.muted,
      'editorLineNumber.activeForeground': theme.colors.text.secondary,
      'editor.selectionBackground': theme.colors.surface.active,
      'editorCursor.foreground': theme.colors.accent.primary,
    }
  };
}

export function hexToHsl(hex: string): { h: number; s: number; l: number } {
  // Conversione HEX → HSL per generare varianti colore
  // ... implementazione
}

export function generateColorVariants(baseColor: string): {
  lighter: string;
  light: string;
  DEFAULT: string;
  dark: string;
  darker: string;
} {
  // Genera 5 varianti da un colore base
  // ... implementazione
}
```

### 6.3 Theme Registry (Built-in Themes)

```typescript
// src/themes/themeRegistry.ts

import { Theme } from '../types/theme';
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
    return this.presets.get('tabularis-dark') || tabularisDark;
  }
}

export const themeRegistry = new ThemeRegistry();
```

---

## 7. UI Components

### 7.1 Theme Settings Page

```typescript
// src/pages/ThemeSettings.tsx

export const ThemeSettings: React.FC = () => {
  const { t } = useTranslation();
  const { 
    currentTheme, 
    settings, 
    allThemes, 
    setTheme, 
    createCustomTheme,
    deleteCustomTheme,
    importTheme,
    exportTheme,
    updateSettings 
  } = useTheme();
  
  const [showEditor, setShowEditor] = useState(false);
  const [editingTheme, setEditingTheme] = useState<Theme | null>(null);
  
  const presets = allThemes.filter(t => t.isPreset);
  const customs = allThemes.filter(t => !t.isPreset);
  
  return (
    <div className="p-6 max-w-4xl">
      <h1 className="text-2xl font-bold mb-6">{t('settings.theme.title')}</h1>
      
      {/* Active Theme */}
      <section className="mb-8">
        <h2 className="text-lg font-semibold mb-4">
          {t('settings.theme.current')}
        </h2>
        <div className="flex items-center gap-4 p-4 bg-surface-secondary rounded-lg">
          <div 
            className="w-16 h-16 rounded-lg"
            style={{ 
              background: currentTheme.colors.bg.base,
              border: `2px solid ${currentTheme.colors.accent.primary}`
            }}
          />
          <div>
            <p className="font-medium">{currentTheme.name}</p>
            <p className="text-sm text-text-muted">
              {currentTheme.isPreset ? t('theme.preset') : t('theme.custom')}
            </p>
          </div>
        </div>
      </section>
      
      {/* Auto-switch */}
      <section className="mb-8">
        <label className="flex items-center gap-3">
          <input
            type="checkbox"
            checked={settings.followSystemTheme}
            onChange={(e) => updateSettings({ followSystemTheme: e.target.checked })}
            className="w-4 h-4"
          />
          <span>{t('settings.theme.followSystem')}</span>
        </label>
        
        {settings.followSystemTheme && (
          <div className="mt-4 ml-7 space-y-3">
            <div>
              <label className="block text-sm mb-1">
                {t('settings.theme.lightTheme')}
              </label>
              <ThemeSelector
                themes={allThemes.filter(t => isLightTheme(t))}
                value={settings.lightThemeId}
                onChange={(id) => updateSettings({ lightThemeId: id })}
              />
            </div>
            <div>
              <label className="block text-sm mb-1">
                {t('settings.theme.darkTheme')}
              </label>
              <ThemeSelector
                themes={allThemes.filter(t => isDarkTheme(t))}
                value={settings.darkThemeId}
                onChange={(id) => updateSettings({ darkThemeId: id })}
              />
            </div>
          </div>
        )}
      </section>
      
      {/* Preset Themes */}
      <section className="mb-8">
        <h2 className="text-lg font-semibold mb-4">
          {t('settings.theme.presets')}
        </h2>
        <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4">
          {presets.map(theme => (
            <ThemeCard
              key={theme.id}
              theme={theme}
              isActive={theme.id === currentTheme.id}
              onClick={() => setTheme(theme.id)}
            />
          ))}
        </div>
      </section>
      
      {/* Custom Themes */}
      <section className="mb-8">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold">
            {t('settings.theme.customThemes')}
          </h2>
          <button
            onClick={() => setShowEditor(true)}
            className="px-4 py-2 bg-accent-primary text-white rounded-lg hover:opacity-90"
          >
            {t('settings.theme.createNew')}
          </button>
        </div>
        
        {customs.length === 0 ? (
          <p className="text-text-muted">
            {t('settings.theme.noCustomThemes')}
          </p>
        ) : (
          <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4">
            {customs.map(theme => (
              <ThemeCard
                key={theme.id}
                theme={theme}
                isActive={theme.id === currentTheme.id}
                onClick={() => setTheme(theme.id)}
                onEdit={() => {
                  setEditingTheme(theme);
                  setShowEditor(true);
                }}
                onDelete={() => deleteCustomTheme(theme.id)}
                onExport={() => handleExport(theme.id)}
              />
            ))}
          </div>
        )}
      </section>
      
      {/* Import */}
      <section>
        <h2 className="text-lg font-semibold mb-4">
          {t('settings.theme.import')}
        </h2>
        <button
          onClick={handleImport}
          className="px-4 py-2 border border-border-default rounded-lg hover:bg-surface-hover"
        >
          {t('settings.theme.importTheme')}
        </button>
      </section>
      
      {/* Theme Editor Modal */}
      {showEditor && (
        <ThemeEditorModal
          theme={editingTheme}
          onClose={() => {
            setShowEditor(false);
            setEditingTheme(null);
          }}
          onSave={(theme) => {
            if (editingTheme) {
              updateCustomTheme(theme);
            } else {
              createCustomTheme(theme.id, theme.name);
            }
            setShowEditor(false);
          }}
        />
      )}
    </div>
  );
};
```

### 7.2 Theme Editor Modal

```typescript
// src/components/modals/ThemeEditorModal.tsx

interface ThemeEditorModalProps {
  theme: Theme | null;  // null = creazione nuovo
  onClose: () => void;
  onSave: (theme: Theme) => void;
}

export const ThemeEditorModal: React.FC<ThemeEditorModalProps> = ({
  theme,
  onClose,
  onSave
}) => {
  const { t } = useTranslation();
  const { allThemes } = useTheme();
  const [activeTab, setActiveTab] = useState<'general' | 'colors' | 'monaco'>('general');
  const [editedTheme, setEditedTheme] = useState<Theme>(() => {
    if (theme) return { ...theme };
    
    // Nuovo tema basato su quello attivo
    const base = allThemes.find(t => t.id === 'tabularis-dark')!;
    return {
      ...base,
      id: `custom-${Date.now()}`,
      name: t('theme.newThemeName'),
      isPreset: false,
      isReadOnly: false
    };
  });
  
  const updateColor = (path: string, value: string) => {
    setEditedTheme(prev => {
      const newTheme = { ...prev };
      const keys = path.split('.');
      let target: any = newTheme;
      for (let i = 0; i < keys.length - 1; i++) {
        target = target[keys[i]];
      }
      target[keys[keys.length - 1]] = value;
      return newTheme;
    });
  };
  
  return (
    <Modal onClose={onClose} size="xl">
      <div className="flex flex-col h-[80vh]">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-border-default">
          <h2 className="text-xl font-semibold">
            {theme ? t('theme.edit') : t('theme.create')}
          </h2>
          <div className="flex gap-2">
            <button
              onClick={onClose}
              className="px-4 py-2 text-text-secondary hover:text-text-primary"
            >
              {t('common.cancel')}
            </button>
            <button
              onClick={() => onSave(editedTheme)}
              className="px-4 py-2 bg-accent-primary text-white rounded-lg"
            >
              {t('common.save')}
            </button>
          </div>
        </div>
        
        {/* Tabs */}
        <div className="flex border-b border-border-default">
          {(['general', 'colors', 'monaco'] as const).map(tab => (
            <button
              key={tab}
              onClick={() => setActiveTab(tab)}
              className={`
                px-4 py-3 font-medium border-b-2 transition-colors
                ${activeTab === tab 
                  ? 'border-accent-primary text-accent-primary' 
                  : 'border-transparent text-text-secondary hover:text-text-primary'
                }
              `}
            >
              {t(`themeEditor.tabs.${tab}`)}
            </button>
          ))}
        </div>
        
        {/* Content */}
        <div className="flex-1 overflow-auto p-6">
          {activeTab === 'general' && (
            <div className="space-y-4">
              <div>
                <label className="block text-sm mb-1">
                  {t('themeEditor.name')}
                </label>
                <input
                  type="text"
                  value={editedTheme.name}
                  onChange={(e) => setEditedTheme(prev => ({ ...prev, name: e.target.value }))}
                  className="w-full px-3 py-2 bg-bg-input rounded-lg border border-border-default"
                />
              </div>
              
              <div>
                <label className="block text-sm mb-1">
                  {t('themeEditor.baseTheme')}
                </label>
                <select
                  value={editedTheme.id.includes('custom') ? '' : editedTheme.id}
                  onChange={(e) => {
                    const base = allThemes.find(t => t.id === e.target.value);
                    if (base) {
                      setEditedTheme(prev => ({
                        ...base,
                        id: prev.id,
                        name: prev.name,
                        isPreset: false,
                        isReadOnly: false
                      }));
                    }
                  }}
                  className="w-full px-3 py-2 bg-bg-input rounded-lg border border-border-default"
                >
                  <option value="">{t('themeEditor.selectBase')}</option>
                  {allThemes.filter(t => t.isPreset).map(t => (
                    <option key={t.id} value={t.id}>{t.name}</option>
                  ))}
                </select>
              </div>
            </div>
          )}
          
          {activeTab === 'colors' && (
            <div className="space-y-6">
              {/* Background Colors */}
              <section>
                <h3 className="font-medium mb-3">{t('themeEditor.colors.background')}</h3>
                <div className="grid grid-cols-2 md:grid-cols-3 gap-4">
                  <ColorPicker
                    label={t('themeEditor.colors.bgBase')}
                    value={editedTheme.colors.bg.base}
                    onChange={(v) => updateColor('colors.bg.base', v)}
                  />
                  <ColorPicker
                    label={t('themeEditor.colors.bgElevated')}
                    value={editedTheme.colors.bg.elevated}
                    onChange={(v) => updateColor('colors.bg.elevated', v)}
                  />
                  <ColorPicker
                    label={t('themeEditor.colors.bgOverlay')}
                    value={editedTheme.colors.bg.overlay}
                    onChange={(v) => updateColor('colors.bg.overlay', v)}
                  />
                </div>
              </section>
              
              {/* Surface Colors */}
              <section>
                <h3 className="font-medium mb-3">{t('themeEditor.colors.surface')}</h3>
                <div className="grid grid-cols-2 md:grid-cols-3 gap-4">
                  <ColorPicker
                    label={t('themeEditor.colors.surfacePrimary')}
                    value={editedTheme.colors.surface.primary}
                    onChange={(v) => updateColor('colors.surface.primary', v)}
                  />
                  <ColorPicker
                    label={t('themeEditor.colors.surfaceSecondary')}
                    value={editedTheme.colors.surface.secondary}
                    onChange={(v) => updateColor('colors.surface.secondary', v)}
                  />
                  <ColorPicker
                    label={t('themeEditor.colors.surfaceHover')}
                    value={editedTheme.colors.surface.hover}
                    onChange={(v) => updateColor('colors.surface.hover', v)}
                  />
                </div>
              </section>
              
              {/* Text Colors */}
              <section>
                <h3 className="font-medium mb-3">{t('themeEditor.colors.text')}</h3>
                <div className="grid grid-cols-2 md:grid-cols-3 gap-4">
                  <ColorPicker
                    label={t('themeEditor.colors.textPrimary')}
                    value={editedTheme.colors.text.primary}
                    onChange={(v) => updateColor('colors.text.primary', v)}
                  />
                  <ColorPicker
                    label={t('themeEditor.colors.textSecondary')}
                    value={editedTheme.colors.text.secondary}
                    onChange={(v) => updateColor('colors.text.secondary', v)}
                  />
                  <ColorPicker
                    label={t('themeEditor.colors.textMuted')}
                    value={editedTheme.colors.text.muted}
                    onChange={(v) => updateColor('colors.text.muted', v)}
                  />
                </div>
              </section>
              
              {/* Accents */}
              <section>
                <h3 className="font-medium mb-3">{t('themeEditor.colors.accents')}</h3>
                <div className="grid grid-cols-2 md:grid-cols-3 gap-4">
                  <ColorPicker
                    label={t('themeEditor.colors.accentPrimary')}
                    value={editedTheme.colors.accent.primary}
                    onChange={(v) => updateColor('colors.accent.primary', v)}
                  />
                  <ColorPicker
                    label={t('themeEditor.colors.accentSuccess')}
                    value={editedTheme.colors.accent.success}
                    onChange={(v) => updateColor('colors.accent.success', v)}
                  />
                  <ColorPicker
                    label={t('themeEditor.colors.accentError')}
                    value={editedTheme.colors.accent.error}
                    onChange={(v) => updateColor('colors.accent.error', v)}
                  />
                </div>
              </section>
            </div>
          )}
          
          {activeTab === 'monaco' && (
            <MonacoThemeEditor
              theme={editedTheme}
              onChange={setEditedTheme}
            />
          )}
        </div>
        
        {/* Preview */}
        <div className="h-48 border-t border-border-default p-4 bg-bg-base">
          <ThemePreview theme={editedTheme} />
        </div>
      </div>
    </Modal>
  );
};
```

### 7.3 Color Picker Component

```typescript
// src/components/ui/ColorPicker.tsx

interface ColorPickerProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
}

export const ColorPicker: React.FC<ColorPickerProps> = ({ label, value, onChange }) => {
  const [isOpen, setIsOpen] = useState(false);
  const pickerRef = useRef<HTMLDivElement>(null);
  
  useClickOutside(pickerRef, () => setIsOpen(false));
  
  return (
    <div className="space-y-1">
      <label className="text-sm text-text-secondary">{label}</label>
      <div className="flex items-center gap-2">
        <button
          onClick={() => setIsOpen(!isOpen)}
          className="w-10 h-10 rounded-lg border border-border-default overflow-hidden"
          style={{ backgroundColor: value }}
        />
        <input
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="flex-1 px-2 py-1 text-sm bg-bg-input rounded border border-border-default font-mono"
        />
      </div>
      
      {isOpen && (
        <div 
          ref={pickerRef}
          className="absolute z-50 mt-1 p-3 bg-bg-elevated rounded-lg shadow-lg border border-border-default"
        >
          {/* Preset color palette */}
          <div className="grid grid-cols-8 gap-1 mb-3">
            {PRESET_COLORS.map(color => (
              <button
                key={color}
                onClick={() => {
                  onChange(color);
                  setIsOpen(false);
                }}
                className="w-6 h-6 rounded"
                style={{ backgroundColor: color }}
              />
            ))}
          </div>
          
          {/* Native color picker */}
          <input
            type="color"
            value={value}
            onChange={(e) => onChange(e.target.value)}
            className="w-full"
          />
        </div>
      )}
    </div>
  );
};
```

---

## 8. Integration Monaco Editor

### 8.1 Monaco Theme Loader

```typescript
// src/components/editor/MonacoEditor.tsx

import Editor from '@monaco-editor/react';
import { useTheme } from '../../contexts/ThemeContext';

export const MonacoEditor: React.FC = () => {
  const { currentTheme, getMonacoThemeJson } = useTheme();
  
  const handleEditorBeforeMount = (monaco: typeof import('monaco-editor')) => {
    // Definisci tema all'avvio
    const themeJson = getMonacoThemeJson();
    monaco.editor.defineTheme(currentTheme.id, themeJson);
  };
  
  const handleEditorDidMount = (editor: any, monaco: any) => {
    // Applica tema attivo
    monaco.editor.setTheme(currentTheme.id);
  };
  
  return (
    <Editor
      beforeMount={handleEditorBeforeMount}
      onMount={handleEditorDidMount}
      theme={currentTheme.id}
      // ... altre props
    />
  );
};
```

---

## 9. Taskbar Icon Implementation

### 9.1 Tauri Icon Update (Limitazioni)

```typescript
// src/utils/taskbarIcon.ts

import { invoke } from '@tauri-apps/api/core';

export async function updateTaskbarIcon(theme: Theme): Promise<void> {
  if (!theme.taskbarIcon) return;
  
  try {
    await invoke('set_taskbar_icon', {
      iconConfig: {
        themeId: theme.id,
        color: theme.taskbarIcon.color
      }
    });
  } catch (error) {
    console.error('Failed to update taskbar icon:', error);
  }
}
```

**Nota importante**: Tauri v2 ha limitazioni nel cambio icona runtime:
- **Windows**: Possibile via `window.set_icon()` ma richiede riavvio per taskbar
- **macOS**: Richiede binding Objective-C a `NSApp`
- **Linux**: Generalmente supportato

**Soluzione alternativa**: Documentare che l'icona taskbar richiede riavvio app, oppure implementare come "nice to have" futuro.

---

## 10. Piano di Implementazione

### Fase 1: Infrastruttura Base (Week 1)
- [ ] Creare `src/types/theme.ts` con tutte le interfacce
- [ ] Creare `src/themes/themeRegistry.ts` con preset built-in
- [ ] Creare `src/themes/presets/` con almeno 5 temi (Tabularis Dark, Light, Monokai, One Dark, Nord)
- [ ] Aggiornare `tailwind.config.js` con color scheme basato su CSS vars
- [ ] Sostituire colori hardcoded in `index.css` con CSS variables
- [ ] Testare che UI continui a funzionare correttamente

### Fase 2: ThemeContext & Backend (Week 1-2)
- [ ] Creare `src/contexts/ThemeContext.tsx` con provider
- [ ] Implementare `applyThemeToCSS()` per applicazione dinamica
- [ ] Aggiornare `SettingsContext` con `ThemeSettings`
- [ ] Creare `src-tauri/src/theme_commands.rs` con comandi base
- [ ] Implementare persistenza temi custom su disco
- [ ] Integrare Monaco theme generation

### Fase 3: UI Settings (Week 2)
- [ ] Creare `src/pages/ThemeSettings.tsx`
- [ ] Implementare `ThemeCard` component
- [ ] Implementare `ThemeSelector` dropdown
- [ ] Aggiungere routing per pagina temi
- [ ] Integrare traduzioni i18n

### Fase 4: Theme Editor (Week 3)
- [ ] Creare `ThemeEditorModal` con tabs
- [ ] Implementare `ColorPicker` component
- [ ] Implementare `ThemePreview` component
- [ ] Aggiungere import/export temi
- [ ] Aggiungere duplicazione temi

### Fase 5: Polish & Testing (Week 3-4)
- [ ] Aggiungere transizioni CSS per cambio tema
- [ ] Implementare follow system theme
- [ ] Testare tutti i preset temi
- [ ] Verificare syntax highlighting per ogni tema
- [ ] Documentazione utente
- [ ] Export tema come JSON shareable

### Fase 6: Taskbar Icon (Future)
- [ ] Ricercare soluzione Tauri per cambio icona runtime
- [ ] Implementare generazione icona SVG dinamica
- [ ] Test su Windows/macOS/Linux

---

## 11. Database Schema Colors (Bonus)

Per evidenziare tipi di dati SQL nella sidebar e nel DataGrid:

```typescript
// Hook per ottenere colore tipo dato
export function useDataTypeColor(type: string): string {
  const { currentTheme } = useTheme();
  const normalizedType = type.toUpperCase();
  
  if (/^(VARCHAR|CHAR|TEXT|STRING)/.test(normalizedType)) {
    return currentTheme.colors.semantic.string;
  }
  if (/^(INT|BIGINT|SMALLINT|FLOAT|DOUBLE|DECIMAL|NUMERIC)/.test(normalizedType)) {
    return currentTheme.colors.semantic.number;
  }
  if (/^(BOOL|BOOLEAN)/.test(normalizedType)) {
    return currentTheme.colors.semantic.boolean;
  }
  if (/^(DATE|TIME|DATETIME|TIMESTAMP)/.test(normalizedType)) {
    return currentTheme.colors.semantic.date;
  }
  
  return currentTheme.colors.text.secondary;
}
```

---

## 12. Conclusioni

Questo sistema di temi offre:

1. **Flessibilità massima**: Ogni aspetto UI personalizzabile
2. **Preset di qualità**: Temi popolari pronti all'uso
3. **Community**: Import/export permette condivisione temi
4. **Performance**: CSS variables per cambio istantaneo
5. **Accessibilità**: Supporto high contrast e tema OS
6. **Future-proof**: Architettura estensibile per nuove feature

Il documento può essere usato come guida implementativa completa e punto di riferimento durante lo sviluppo.
