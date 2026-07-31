import type { DriverCapabilities } from '../types/plugins';
import { stripLeadingSqlComments } from './sql';

export interface TableDataChangeScope {
  schema?: string;
  database?: string;
}

/**
 * Returns true when a driver supports cross-database access from a single connection
 * (e.g. MySQL). Postgres uses schemas; SQLite/DuckDB are file-based or folder-based.
 */
export function isMultiDatabaseCapable(capabilities: DriverCapabilities | null | undefined): boolean {
  if (!capabilities) return false;
  if (capabilities.no_connection_required) return false;
  if (capabilities.console_only) return false;
  // A flat single-database store (e.g. Meilisearch) has nothing to select.
  if (capabilities.single_database) return false;
  return (
    capabilities.file_based === false &&
    !capabilities.folder_based &&
    capabilities.schemas === false
  );
}

export function getTableDataChangeScope(
  capabilities: DriverCapabilities | null | undefined,
  tabSchema: string | null | undefined,
  activeSchema: string | null | undefined,
): TableDataChangeScope {
  if (isMultiDatabaseCapable(capabilities) && tabSchema) {
    return { database: tabSchema };
  }

  if (capabilities?.schemas === true) {
    const schema = tabSchema ?? activeSchema;
    return schema ? { schema } : {};
  }

  return {};
}

/**
 * Returns true when the database param is an array (multi-database selection).
 */
export function isMultiDatabaseSelection(db: string | string[]): db is string[] {
  return Array.isArray(db);
}

/**
 * Normalizes a database param (string or string[]) into an array of database names.
 * An empty string or empty array returns an empty array.
 */
export function getDatabaseList(db: string | string[]): string[] {
  if (Array.isArray(db)) {
    return db;
  }
  return db ? [db] : [];
}

/**
 * Returns the primary (first) database name from a string or string[].
 * Falls back to '' when the array is empty or the string is empty.
 */
export function getEffectiveDatabase(db: string | string[]): string {
  if (Array.isArray(db)) {
    return db[0] ?? '';
  }
  return db;
}

/**
 * Reconciles a saved database selection against the databases that actually
 * exist on the server. Preserves the saved order; entries that no longer
 * exist are reported in `removed` so callers can persist the pruned list.
 */
export function reconcileDatabaseSelection(
  saved: string[],
  available: string[],
): { selection: string[]; removed: string[] } {
  const availableSet = new Set(available);
  const selection: string[] = [];
  const removed: string[] = [];
  for (const db of saved) {
    if (availableSet.has(db)) {
      selection.push(db);
    } else {
      removed.push(db);
    }
  }
  return { selection, removed };
}

function nonBlank(value: string | null | undefined): string | undefined {
  const normalized = value?.trim();
  return normalized ? normalized : undefined;
}

/**
 * Resolve the scope shown by the editor and sent to the backend from the same
 * precedence chain. A tab opened for a specific database is authoritative;
 * multi-database tabs without a pinned scope use the current/visible database,
 * never an implicit connection default.
 */
export function resolveExecutionScope(
  tabScope: string | null | undefined,
  activeScope: string | null | undefined,
  selectedDatabases: readonly string[],
  isMultiDatabase: boolean,
): string | undefined {
  const pinned = nonBlank(tabScope);
  if (pinned) return pinned;

  const active = nonBlank(activeScope);
  if (active) return active;

  if (isMultiDatabase) {
    return selectedDatabases.map(nonBlank).find((database) => database !== undefined);
  }

  return undefined;
}

/** Keep the active database valid when the selected database set changes. */
export function resolveActiveDatabase(
  selectedDatabases: readonly string[],
  currentDatabase: string | null | undefined,
): string | null {
  const current = nonBlank(currentDatabase);
  if (current && selectedDatabases.includes(current)) return current;
  return selectedDatabases.map(nonBlank).find((database) => database !== undefined) ?? null;
}

/** Whether successful SQL can add, remove, or rename a database catalog entry. */
export function changesDatabaseCatalog(sql: string): boolean {
  return /\b(?:CREATE|DROP|ALTER)\s+(?:DATABASE|SCHEMA)\b/i.test(sql);
}

/** Extract the target from a standalone MySQL `USE db_name` statement. */
export function parseUseDatabaseStatement(sql: string): string | null {
  const statement = stripLeadingSqlComments(sql).trim();
  const match = /^USE\s+(?:`((?:``|[^`\r\n])+)`|([A-Za-z0-9_$]+))\s*;?\s*$/i.exec(
    statement,
  );
  if (!match) return null;
  return match[1] !== undefined ? match[1].replace(/``/g, '`') : match[2];
}

export interface UseDatabaseSwitch {
  database: string;
  shouldSwitch: boolean;
  shouldAddToSelection: boolean;
}

/** Decide how a successful `USE` statement updates only its current console. */
export function resolveUseDatabaseSwitch(
  sql: string,
  currentDatabase: string | null | undefined,
  selectedDatabases: readonly string[],
): UseDatabaseSwitch | null {
  const database = parseUseDatabaseStatement(sql);
  if (!database) return null;

  return {
    database,
    shouldSwitch: nonBlank(currentDatabase) !== database,
    shouldAddToSelection: !selectedDatabases.includes(database),
  };
}
