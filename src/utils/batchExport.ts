import type { QueryResultEntry } from "../types/editor";

export function getExportableResultEntries(
  results: QueryResultEntry[] | undefined,
): QueryResultEntry[] {
  return (results ?? []).filter(
    (entry) =>
      !entry.isLoading &&
      !entry.error &&
      Boolean(entry.result?.columns.length) &&
      Boolean(entry.query.trim()),
  );
}

function safeFileStem(entry: QueryResultEntry): string {
  const fallback = `result_${entry.queryIndex + 1}`;
  const sanitized = [...(entry.label?.trim() || fallback)]
    .map((character) =>
      character.charCodeAt(0) < 32 || '<>:"/\\|?*'.includes(character)
        ? "_"
        : character,
    )
    .join("")
    .replace(/[. ]+$/g, "")
    .slice(0, 80);
  return sanitized || fallback;
}

export function buildBatchExportFileName(
  entry: QueryResultEntry,
  listIndex: number,
  extension: string,
): string {
  const prefix = String(listIndex + 1).padStart(2, "0");
  return `${prefix}_${safeFileStem(entry)}.${extension}`;
}
