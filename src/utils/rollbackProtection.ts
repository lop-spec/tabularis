import { leadingKeyword } from "./sqlSplitter/classify";

export const ROLLBACK_RISK_REVIEW_PREFIX =
  "TABULARIS_ROLLBACK_RISK_REVIEW:";

export type RollbackUnsupportedPolicy = "skip" | "execute_unprotected";
export type RollbackRiskDecision =
  | RollbackUnsupportedPolicy
  | "allow_implicit_commit";
export type RollbackRiskKind = "unsupported" | "implicit_commit";

export interface RollbackRiskStatement {
  index: number;
  sql: string;
  reason: string;
  destructive: boolean;
}

export interface RollbackRiskReview {
  kind: RollbackRiskKind;
  statements: RollbackRiskStatement[];
}

const CLEARLY_READ_ONLY_KEYWORDS = new Set([
  "SELECT",
  "SHOW",
  "DESCRIBE",
  "DESC",
  "EXPLAIN",
  "VALUES",
  "TABLE",
  "PRAGMA",
]);

/**
 * Returns whether a single statement must use the rollback-aware batch path.
 *
 * The allow-list is intentionally narrow: ambiguous families such as WITH,
 * CALL, SET, USE, and unknown vendor statements are delegated to the backend
 * rollback planner, which can classify them accurately and fail closed.
 */
export function requiresRollbackProtectedExecution(sql: string): boolean {
  const keyword = leadingKeyword(sql);
  if (keyword) return !CLEARLY_READ_ONLY_KEYWORDS.has(keyword);

  // MySQL executable comments are meaningful statements, but the generic
  // leading-keyword helper deliberately treats block comments as non-code.
  return /\/\*!/.test(sql);
}

function errorMessage(error: unknown): string | null {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return null;
}

export function parseRollbackRiskReview(
  error: unknown,
): RollbackRiskReview | null {
  const message = errorMessage(error);
  if (!message) return null;
  const prefixIndex = message.indexOf(ROLLBACK_RISK_REVIEW_PREFIX);
  if (prefixIndex < 0) return null;

  try {
    const parsed = JSON.parse(
      message.slice(prefixIndex + ROLLBACK_RISK_REVIEW_PREFIX.length),
    ) as Partial<RollbackRiskReview>;
    if (!Array.isArray(parsed.statements) || parsed.statements.length === 0) {
      return null;
    }
    const kind = parsed.kind ?? "unsupported";
    if (kind !== "unsupported" && kind !== "implicit_commit") {
      return null;
    }
    const valid = parsed.statements.every(
      (statement) =>
        Number.isInteger(statement?.index) &&
        statement.index > 0 &&
        typeof statement.sql === "string" &&
        statement.sql.trim().length > 0 &&
        typeof statement.reason === "string" &&
        statement.reason.trim().length > 0 &&
        typeof statement.destructive === "boolean",
    );
    return valid
      ? ({ ...parsed, kind } as RollbackRiskReview)
      : null;
  } catch {
    return null;
  }
}
