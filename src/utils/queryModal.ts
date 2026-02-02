/**
 * Query Modal Utilities
 * Pure functions for form validation and state management
 */

export interface QueryFormData {
  name: string;
  sql: string;
}

export interface ValidationResult {
  valid: boolean;
  error?: string;
}

/**
 * Validates query form data
 */
export function validateQueryForm(data: QueryFormData): ValidationResult {
  if (!data.name || !data.name.trim()) {
    return { valid: false, error: 'Name is required' };
  }

  if (!data.sql || !data.sql.trim()) {
    return { valid: false, error: 'SQL content is required' };
  }

  return { valid: true };
}

/**
 * Trims form values and returns cleaned data
 */
export function cleanQueryFormData(data: QueryFormData): QueryFormData {
  return {
    name: data.name.trim(),
    sql: data.sql.trim(),
  };
}

/**
 * Checks if SQL content has meaningful content (not just whitespace/comments)
 */
export function hasMeaningfulSql(sql: string): boolean {
  const cleaned = sql
    .replace(/--[^\n]*/g, '') // Remove line comments
    .replace(/\/\*[\s\S]*?\*\//g, '') // Remove block comments
    .replace(/\s+/g, ' ') // Normalize whitespace
    .trim();

  return cleaned.length > 0;
}

/**
 * Validates SQL syntax (basic checks)
 */
export function validateSqlSyntax(sql: string): ValidationResult {
  const trimmed = sql.trim();

  if (!trimmed) {
    return { valid: false, error: 'SQL cannot be empty' };
  }

  // Check for common SQL injection patterns (basic)
  const dangerousPatterns = [
    /;\s*drop\s+/i,
    /;\s*delete\s+/i,
    /;\s*truncate\s+/i,
  ];

  for (const pattern of dangerousPatterns) {
    if (pattern.test(trimmed)) {
      return { valid: false, error: 'Potentially dangerous SQL pattern detected' };
    }
  }

  return { valid: true };
}
