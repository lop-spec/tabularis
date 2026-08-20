import type { BatchStatementResult, Tab } from "../types/editor";

export function findRollbackFile(
  results: BatchStatementResult[],
): string | undefined {
  return results
    .map((result) => result.rollback_file)
    .find(
      (path): path is string =>
        typeof path === "string" && path.trim().length > 0,
    );
}

export function createRollbackViewerTab(
  query: string,
  schema?: string,
): Partial<Tab> {
  return {
    type: "console",
    title: "rollback.sql",
    query,
    readOnly: true,
    ...(schema ? { schema } : {}),
  };
}
