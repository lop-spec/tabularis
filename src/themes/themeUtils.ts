import type { Theme, MonacoThemeDefinition } from "../types/theme";
import * as monaco from "monaco-editor";

// Static imports for monaco-themes (copied locally for Vite compatibility)
import MonokaiTheme from "./monaco/Monokai.json";
import DraculaTheme from "./monaco/Dracula.json";
import NordTheme from "./monaco/Nord.json";
import GitHubDarkTheme from "./monaco/GitHub Dark.json";
import GitHubLightTheme from "./monaco/GitHub Light.json";
import SolarizedDarkTheme from "./monaco/Solarized-dark.json";
import SolarizedLightTheme from "./monaco/Solarized-light.json";
import OneDarkProTheme from "./monaco/One Dark Pro.json";
import NightOwlTheme from "./monaco/Night Owl.json";

// Map of theme names to imported theme data
const MONACO_THEMES_MAP: Record<string, monaco.editor.IStandaloneThemeData> = {
  Monokai: MonokaiTheme as monaco.editor.IStandaloneThemeData,
  Dracula: DraculaTheme as monaco.editor.IStandaloneThemeData,
  Nord: NordTheme as monaco.editor.IStandaloneThemeData,
  "GitHub Dark": GitHubDarkTheme as monaco.editor.IStandaloneThemeData,
  "GitHub Light": GitHubLightTheme as monaco.editor.IStandaloneThemeData,
  "Solarized-dark": SolarizedDarkTheme as monaco.editor.IStandaloneThemeData,
  "Solarized-light": SolarizedLightTheme as monaco.editor.IStandaloneThemeData,
  "One Dark Pro": OneDarkProTheme as monaco.editor.IStandaloneThemeData,
  "Night Owl": NightOwlTheme as monaco.editor.IStandaloneThemeData,
};

// Track which themes have been defined
const definedThemes = new Set<string>();

/**
 * Loads and applies a Monaco Editor theme from the monaco-themes package
 * This is now synchronous to work with beforeMount
 */
export function loadMonacoTheme(
  theme: Theme,
  monacoInstance?: typeof monaco,
): void {
  const themeName = theme.monacoTheme.themeName;
  const monacoToUse = monacoInstance || monaco;

  try {
    // If theme has a themeName, use the predefined Monaco theme
    if (themeName && MONACO_THEMES_MAP[themeName]) {
      const themeData = MONACO_THEMES_MAP[themeName];

      // Define the theme with our theme ID if not already defined
      if (!definedThemes.has(theme.id)) {
        monacoToUse.editor.defineTheme(theme.id, themeData);
        definedThemes.add(theme.id);
      }

      // Set it as the active theme
      monacoToUse.editor.setTheme(theme.id);
    } else {
      // No themeName or not found, use fallback generated theme
      const fallbackTheme = generateMonacoTheme(theme);
      if (!definedThemes.has(theme.id)) {
        monacoToUse.editor.defineTheme(
          theme.id,
          fallbackTheme as monaco.editor.IStandaloneThemeData,
        );
        definedThemes.add(theme.id);
      }
      monacoToUse.editor.setTheme(theme.id);
    }
  } catch (error) {
    console.error(`Failed to load Monaco theme for "${theme.name}":`, error);
    // Final fallback to vs-dark
    monacoToUse.editor.setTheme(theme.monacoTheme.base || "vs-dark");
  }
}

export function applyThemeToCSS(theme: Theme): void {
  const root = document.documentElement;

  // Background
  root.style.setProperty("--bg-base", theme.colors.bg.base);
  root.style.setProperty("--bg-elevated", theme.colors.bg.elevated);
  root.style.setProperty("--bg-overlay", theme.colors.bg.overlay);
  root.style.setProperty("--bg-input", theme.colors.bg.input);
  root.style.setProperty("--bg-tooltip", theme.colors.bg.tooltip);

  // Surface
  root.style.setProperty("--surface-primary", theme.colors.surface.primary);
  root.style.setProperty("--surface-secondary", theme.colors.surface.secondary);
  root.style.setProperty("--surface-tertiary", theme.colors.surface.tertiary);
  root.style.setProperty("--surface-hover", theme.colors.surface.hover);
  root.style.setProperty("--surface-active", theme.colors.surface.active);
  root.style.setProperty("--surface-disabled", theme.colors.surface.disabled);

  // Text
  root.style.setProperty("--text-primary", theme.colors.text.primary);
  root.style.setProperty("--text-secondary", theme.colors.text.secondary);
  root.style.setProperty("--text-muted", theme.colors.text.muted);
  root.style.setProperty("--text-disabled", theme.colors.text.disabled);
  root.style.setProperty("--text-accent", theme.colors.text.accent);
  root.style.setProperty("--text-inverse", theme.colors.text.inverse);

  // Accent
  root.style.setProperty("--accent-primary", theme.colors.accent.primary);
  root.style.setProperty("--accent-secondary", theme.colors.accent.secondary);
  root.style.setProperty("--accent-success", theme.colors.accent.success);
  root.style.setProperty("--accent-warning", theme.colors.accent.warning);
  root.style.setProperty("--accent-error", theme.colors.accent.error);
  root.style.setProperty("--accent-info", theme.colors.accent.info);

  // Border
  root.style.setProperty("--border-subtle", theme.colors.border.subtle);
  root.style.setProperty("--border-default", theme.colors.border.default);
  root.style.setProperty("--border-strong", theme.colors.border.strong);
  root.style.setProperty("--border-focus", theme.colors.border.focus);

  // Semantic
  root.style.setProperty("--semantic-string", theme.colors.semantic.string);
  root.style.setProperty("--semantic-number", theme.colors.semantic.number);
  root.style.setProperty("--semantic-boolean", theme.colors.semantic.boolean);
  root.style.setProperty("--semantic-date", theme.colors.semantic.date);
  root.style.setProperty("--semantic-null", theme.colors.semantic.null);
  root.style.setProperty("--semantic-pk", theme.colors.semantic.primaryKey);
  root.style.setProperty("--semantic-fk", theme.colors.semantic.foreignKey);
  root.style.setProperty("--semantic-index", theme.colors.semantic.index);
  root.style.setProperty("--semantic-modified", theme.colors.semantic.modified);
  root.style.setProperty("--semantic-deleted", theme.colors.semantic.deleted);
  root.style.setProperty("--semantic-new", theme.colors.semantic.new);

  // Typography
  root.style.setProperty("--font-base", theme.typography.fontFamily.base);
  root.style.setProperty("--font-mono", theme.typography.fontFamily.mono);

  // Layout
  root.style.setProperty("--radius-sm", theme.layout.borderRadius.sm);
  root.style.setProperty("--radius-base", theme.layout.borderRadius.base);
  root.style.setProperty("--radius-lg", theme.layout.borderRadius.lg);
  root.style.setProperty("--radius-xl", theme.layout.borderRadius.xl);

  // Color scheme
  const colorScheme = theme.monacoTheme.base === "vs" ? "light" : "dark";
  root.style.setProperty("color-scheme", colorScheme);
}

export function generateMonacoTheme(theme: Theme): MonacoThemeDefinition {
  const baseColors = theme.monacoTheme.colors || {};

  return {
    base: theme.monacoTheme.base,
    inherit: theme.monacoTheme.inherit,
    rules: theme.monacoTheme.rules,
    colors: {
      // Base editor colors
      "editor.background": theme.colors.bg.base,
      "editor.foreground": theme.colors.text.primary,
      "editorCursor.foreground": theme.colors.accent.primary,

      // Selection and highlights
      "editor.selectionBackground": theme.colors.surface.active,
      "editor.inactiveSelectionBackground": `${theme.colors.surface.active}80`,
      "editor.selectionHighlightBackground": `${theme.colors.accent.primary}30`,
      "editor.wordHighlightBackground": `${theme.colors.accent.secondary}30`,
      "editor.wordHighlightStrongBackground": `${theme.colors.accent.primary}40`,

      // Line numbers
      "editorLineNumber.foreground": theme.colors.text.muted,
      "editorLineNumber.activeForeground": theme.colors.text.secondary,

      // Current line highlighting
      "editor.lineHighlightBackground": theme.colors.surface.hover,
      "editor.lineHighlightBorder": "transparent",

      // Find/replace
      "editor.findMatchBackground": `${theme.colors.accent.warning}40`,
      "editor.findMatchHighlightBackground": `${theme.colors.accent.warning}30`,

      // Hover and links
      "editor.hoverHighlightBackground": theme.colors.surface.hover,
      "editorLink.activeForeground": theme.colors.accent.primary,

      // Gutter
      "editorGutter.background": theme.colors.bg.base,
      "editorGutter.modifiedBackground": theme.colors.semantic.modified,
      "editorGutter.addedBackground": theme.colors.semantic.new,
      "editorGutter.deletedBackground": theme.colors.semantic.deleted,

      // Error/Warning/Info squiggles and borders - make them less intrusive
      "editorError.foreground": theme.colors.accent.error,
      "editorError.background": "transparent",
      "editorError.border": "transparent",
      "editorWarning.foreground": theme.colors.accent.warning,
      "editorWarning.background": "transparent",
      "editorWarning.border": "transparent",
      "editorInfo.foreground": theme.colors.accent.info,
      "editorInfo.background": "transparent",
      "editorInfo.border": "transparent",
      "editorHint.foreground": theme.colors.text.muted,
      "editorHint.background": "transparent",
      "editorHint.border": "transparent",

      // Input validation
      "inputValidation.errorBackground": `${theme.colors.accent.error}20`,
      "inputValidation.errorForeground": theme.colors.accent.error,
      "inputValidation.errorBorder": "transparent",
      "inputValidation.warningBackground": `${theme.colors.accent.warning}20`,
      "inputValidation.warningForeground": theme.colors.accent.warning,
      "inputValidation.warningBorder": "transparent",
      "inputValidation.infoBackground": `${theme.colors.accent.info}20`,
      "inputValidation.infoForeground": theme.colors.accent.info,
      "inputValidation.infoBorder": "transparent",

      // Widgets (autocomplete, etc)
      "editorWidget.background": theme.colors.surface.primary,
      "editorWidget.border": theme.colors.border.default,
      "editorWidget.foreground": theme.colors.text.primary,

      // Suggest widget (autocomplete)
      "editorSuggestWidget.background": theme.colors.surface.primary,
      "editorSuggestWidget.border": theme.colors.border.default,
      "editorSuggestWidget.foreground": theme.colors.text.primary,
      "editorSuggestWidget.highlightForeground": theme.colors.accent.primary,
      "editorSuggestWidget.selectedBackground": theme.colors.surface.active,
      "editorSuggestWidget.selectedForeground": theme.colors.text.primary,

      // Peek view
      "peekView.background": theme.colors.surface.secondary,
      "peekViewEditor.background": theme.colors.bg.elevated,
      "peekViewEditorGutter.background": theme.colors.bg.elevated,
      "peekViewResult.background": theme.colors.surface.secondary,
      "peekView.border": theme.colors.border.default,

      // Minimap
      "minimap.background": theme.colors.surface.primary,
      "minimap.findMatchHighlight": theme.colors.accent.warning,
      "minimap.selectionHighlight": theme.colors.accent.primary,

      // Scrollbar
      "scrollbarSlider.background": `${theme.colors.surface.tertiary}80`,
      "scrollbarSlider.hoverBackground": `${theme.colors.surface.tertiary}99`,
      "scrollbarSlider.activeBackground": theme.colors.surface.tertiary,

      // Overview ruler
      "editorOverviewRuler.background": theme.colors.bg.base,
      "editorOverviewRuler.border": "transparent",

      // Input controls
      "input.background": theme.colors.bg.input,
      "input.border": theme.colors.border.default,
      "input.foreground": theme.colors.text.primary,
      "input.placeholderForeground": theme.colors.text.muted,

      // Buttons
      "button.background": theme.colors.accent.primary,
      "button.foreground": theme.colors.text.inverse,
      "button.hoverBackground": lighten(theme.colors.accent.primary, 0.1),

      // Badge
      "badge.background": theme.colors.accent.primary,
      "badge.foreground": theme.colors.text.inverse,

      // List/Tree (used in some widgets)
      "list.activeSelectionBackground": theme.colors.surface.active,
      "list.activeSelectionForeground": theme.colors.text.primary,
      "list.hoverBackground": theme.colors.surface.hover,
      "list.hoverForeground": theme.colors.text.primary,
      "list.focusBackground": theme.colors.surface.active,
      "list.focusForeground": theme.colors.text.primary,

      // Menu
      "menu.background": theme.colors.surface.primary,
      "menu.foreground": theme.colors.text.primary,
      "menu.selectionBackground": theme.colors.surface.active,
      "menu.selectionForeground": theme.colors.text.primary,
      "menu.border": theme.colors.border.default,

      // Title bar
      "titleBar.activeBackground": theme.colors.bg.elevated,
      "titleBar.activeForeground": theme.colors.text.primary,
      "titleBar.inactiveBackground": theme.colors.bg.base,
      "titleBar.inactiveForeground": theme.colors.text.muted,

      // Status bar
      "statusBar.background": theme.colors.bg.elevated,
      "statusBar.foreground": theme.colors.text.secondary,
      "statusBar.border": theme.colors.border.default,

      // Existing colors from theme definition
      ...baseColors,
    },
  };
}

export function hexToRgb(
  hex: string,
): { r: number; g: number; b: number } | null {
  const cleanHex = hex.replace("#", "");

  if (cleanHex.length === 3) {
    const r = parseInt(cleanHex[0] + cleanHex[0], 16);
    const g = parseInt(cleanHex[1] + cleanHex[1], 16);
    const b = parseInt(cleanHex[2] + cleanHex[2], 16);
    return { r, g, b };
  }

  if (cleanHex.length === 6) {
    const r = parseInt(cleanHex.substring(0, 2), 16);
    const g = parseInt(cleanHex.substring(2, 4), 16);
    const b = parseInt(cleanHex.substring(4, 6), 16);
    return { r, g, b };
  }

  return null;
}

export function rgbToHex(r: number, g: number, b: number): string {
  const toHex = (n: number): string => {
    const hex = Math.round(n).toString(16);
    return hex.length === 1 ? "0" + hex : hex;
  };
  return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
}

export function lighten(hex: string, amount: number): string {
  const rgb = hexToRgb(hex);
  if (!rgb) return hex;

  const r = Math.min(255, rgb.r + (255 - rgb.r) * amount);
  const g = Math.min(255, rgb.g + (255 - rgb.g) * amount);
  const b = Math.min(255, rgb.b + (255 - rgb.b) * amount);

  return rgbToHex(r, g, b);
}

export function darken(hex: string, amount: number): string {
  const rgb = hexToRgb(hex);
  if (!rgb) return hex;

  const r = Math.max(0, rgb.r * (1 - amount));
  const g = Math.max(0, rgb.g * (1 - amount));
  const b = Math.max(0, rgb.b * (1 - amount));

  return rgbToHex(r, g, b);
}

export function generateColorScale(baseColor: string): {
  50: string;
  100: string;
  200: string;
  300: string;
  400: string;
  500: string;
  600: string;
  700: string;
  800: string;
  900: string;
} {
  return {
    50: lighten(baseColor, 0.9),
    100: lighten(baseColor, 0.8),
    200: lighten(baseColor, 0.6),
    300: lighten(baseColor, 0.4),
    400: lighten(baseColor, 0.2),
    500: baseColor,
    600: darken(baseColor, 0.2),
    700: darken(baseColor, 0.4),
    800: darken(baseColor, 0.6),
    900: darken(baseColor, 0.8),
  };
}
