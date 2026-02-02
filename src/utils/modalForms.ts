/**
 * Query Modal Utilities
 * Pure functions for form validation and state management
 */

export interface QueryFormData {
  name: string;
  sql: string;
}

export interface ValidationResult {
  isValid: boolean;
  error: string | null;
}

/**
 * Validates query form data
 */
export function validateQueryForm(data: QueryFormData): ValidationResult {
  if (!data.name || !data.name.trim()) {
    return { isValid: false, error: 'Name is required' };
  }

  if (!data.sql || !data.sql.trim()) {
    return { isValid: false, error: 'SQL content is required' };
  }

  return { isValid: true, error: null };
}

/**
 * Trims form values
 */
export function trimFormValues(data: QueryFormData): QueryFormData {
  return {
    name: data.name.trim(),
    sql: data.sql.trim(),
  };
}

/**
 * Formats number with locale
 */
export function formatNumberLocale(num: number): string {
  return num.toLocaleString();
}

/**
 * Formats large numbers in compact notation
 */
export function formatCompactNumber(num: number): string {
  if (num < 1000) return num.toString();
  if (num < 1000000) return (num / 1000).toFixed(1) + 'K';
  if (num < 1000000000) return (num / 1000000).toFixed(1) + 'M';
  return (num / 1000000000).toFixed(1) + 'B';
}
