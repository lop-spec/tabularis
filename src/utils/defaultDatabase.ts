import type { ConnectionData } from "../contexts/DatabaseContext";
import { isMultiDatabaseCapable } from "./database";

/** The first selected database is already the persisted connection default. */
export function promoteDefaultDatabase(selected: readonly string[], database: string): string[] {
  if (!database.trim() || !selected.includes(database)) {
    throw new Error("The default database must belong to this connection's selection");
  }
  return [database, ...selected.filter((name) => name !== database)];
}

/** New tabs inherit a default; existing tabs and explicit scopes are untouched. */
export function getNewTabDatabase(data: ConnectionData | undefined): string | undefined {
  if (!data || !isMultiDatabaseCapable(data.capabilities)) return undefined;
  return data.selectedDatabases[0] || data.databaseName || undefined;
}
