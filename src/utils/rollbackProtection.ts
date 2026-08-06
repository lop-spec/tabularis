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
  "VALUES",
  "TABLE",
  "PRAGMA",
]);

// `SELECT … INTO OUTFILE/DUMPFILE` writes server-side files; the backend
// planner blocks it, so it must not slip through the read-only fast path.
const SELECT_FILE_WRITE = /\bINTO\s+(?:OUTFILE|DUMPFILE)\b/i;

// `SET @var = …` only assigns a user variable: no data changes, nothing to
// roll back. Every other SET form (GLOBAL/SESSION/PERSIST, NAMES, autocommit,
// TRANSACTION …) alters server or session behaviour and stays protected, so
// the match is anchored to `@` and rejects `@@system_variables`.
//
// Without this, a batch that merely primed a few counters for a read-only
// pre-flight check pulled the entire batch — the SELECTs included — into the
// pinned-transaction path and raised a risk prompt.
const SET_USER_VARIABLE_ONLY =
  /^SET\s+@(?!@)[A-Za-z0-9_$-￿]+\s*(?::?=)/i;

/**
 * Returns whether a single statement must use the rollback-aware batch path.
 *
 * The allow-list is intentionally narrow: ambiguous families such as WITH,
 * CALL, USE, and unknown vendor statements are delegated to the backend
 * rollback planner, which can classify them accurately and fail closed.
 * EXPLAIN is NOT on the list: MySQL 8.0.18+ `EXPLAIN ANALYZE UPDATE …`
 * really executes the write, so only the backend planner may clear it.
 * `SET` is delegated too, except for plain user-variable assignment.
 */
export function requiresRollbackProtectedExecution(sql: string): boolean {
  const keyword = leadingKeyword(sql);
  if (keyword) {
    if (keyword === "SET") {
      return !SET_USER_VARIABLE_ONLY.test(sql.trim());
    }
    if (!CLEARLY_READ_ONLY_KEYWORDS.has(keyword)) return true;
    return SELECT_FILE_WRITE.test(sql);
  }

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
