/**
 * Settings UI utilities
 * Extracted from Settings.tsx for testability
 */

/**
 * Font size bounds
 */
export const MIN_FONT_SIZE = 10;
export const MAX_FONT_SIZE = 20;

/**
 * Validate font size input
 * @param size - Font size in pixels
 * @returns True if font size is valid
 */
export function validateFontSize(size: number): boolean {
  return Number.isInteger(size) && size >= MIN_FONT_SIZE && size <= MAX_FONT_SIZE;
}

/**
 * Check if a font family is a predefined preset
 * @param fontFamily - Font family name
 * @param availableFonts - List of available font presets
 * @returns True if font is a preset
 */
export function isPresetFont(
  fontFamily: string,
  availableFonts: ReadonlyArray<{ name: string; label: string }>
): boolean {
  return availableFonts.some((f) => f.name === fontFamily);
}

/**
 * Format a roadmap feature for display
 * @param label - Feature label
 * @param done - Whether the feature is completed
 * @returns Formatted display object
 */
export function formatRoadmapFeature(label: string, done: boolean): {
  label: string;
  status: 'completed' | 'pending';
  icon: '✓' | '○';
} {
  return {
    label,
    status: done ? 'completed' : 'pending',
    icon: done ? '✓' : '○',
  };
}
