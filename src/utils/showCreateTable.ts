import { quoteTableRef } from "./identifiers";

interface ShowCreateTableResult {
  columns: string[];
  rows: unknown[][];
}

export function supportsShowCreateTable(driver: string | null | undefined): boolean {
  return driver === "mysql" || driver === "mariadb";
}

export function buildShowCreateTableQuery(
  tableName: string,
  schema?: string | null,
): string {
  return `SHOW CREATE TABLE ${quoteTableRef(tableName, "mysql", schema)};`;
}

export function extractCreateTableSql(result: ShowCreateTableResult): string {
  const createTableIndex = result.columns.findIndex(
    (column) => column.trim().toLowerCase() === "create table",
  );
  const value = createTableIndex >= 0 ? result.rows[0]?.[createTableIndex] : undefined;

  if (typeof value !== "string" || value.trim().length === 0) {
    throw new Error("SHOW CREATE TABLE did not return a CREATE TABLE statement");
  }

  const ddl = value.trim();
  return ddl.endsWith(";") ? ddl : `${ddl};`;
}
