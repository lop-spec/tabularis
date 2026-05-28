// SQL Analysis Utilities - Pure logic functions for parsing and analyzing SQL

// Removes wrapping SQL identifier quotes/backticks.
// Unquoted identifiers are normalized to lowercase.
function stripIdentifierQuotes(token: string): string {
  const q = token[0];
  if (q === '"' || q === '`') return token.slice(1, -1);
  return token.toLowerCase();
}

// Optimized table parser - early exit and minimal allocations
export const parseTablesFromQuery = (sql: string): Map<string, string> | null => {
  if (!sql || sql.length === 0) return null;

  const lowerSql = sql.toLowerCase();

  // Quick check if query contains FROM/JOIN keywords
  if (!lowerSql.includes('from') && !lowerSql.includes('join')) {
    return null;
  }

  // Only scan FROM clause onward (avoids SELECT-list commas; keeps quoted case)
  const fromAt = lowerSql.search(/\bfrom\b/);
  const scan = fromAt >= 0 ? sql.slice(fromAt) : sql;

  const tableMap = new Map<string, string>();
  const fromPattern =
    /(?:from|join|,)\s+("(?:[^"]|"")*"|`[^`]+`|[a-zA-Z_][a-zA-Z0-9_]*)(?:\.("(?:[^"]|"")*"|`[^`]+`|[a-zA-Z_][a-zA-Z0-9_]*))?(?:\s+(?:as\s+)?("(?:[^"]|"")*"|`[^`]+`|[a-zA-Z_][a-zA-Z0-9_]*))?/gi;

  let match;
  let matchCount = 0;
  const MAX_MATCHES = 10; // Prevent regex catastrophic backtracking

  while ((match = fromPattern.exec(scan)) !== null && matchCount++ < MAX_MATCHES) {
    const tableToken = match[2] ?? match[1];

    if (!tableToken) continue;

    const tableName = stripIdentifierQuotes(tableToken);
    const aliasToken = match[3];
    const alias = aliasToken ? stripIdentifierQuotes(aliasToken) : tableName;

    tableMap.set(alias.toLowerCase(), tableName);
  }

  return tableMap.size > 0 ? tableMap : null;
};

// Optimized statement extractor - avoid full text scan when possible
export const getCurrentStatement = (model: { getValue: () => string; getOffsetAt: (position: { lineNumber: number; column: number }) => number }, position: { lineNumber: number; column: number }): string => {
  const fullText = model.getValue();

  // For small files, just return full text
  if (fullText.length < 500) {
    return fullText;
  }

  const offset = model.getOffsetAt(position);
  let start = 0;
  let end = fullText.length;


  // Search within reasonable bounds (±2000 chars from cursor)
  const searchStart = Math.max(0, offset - 2000);
  const searchEnd = Math.min(fullText.length, offset + 2000);

  // Find previous semicolon
  for (let i = offset - 1; i >= searchStart; i--) {
    if (fullText[i] === ';') {
      start = i + 1;
      break;
    }
  }

  // Find next semicolon
  for (let i = offset; i < searchEnd; i++) {
    if (fullText[i] === ';') {
      end = i;
      break;
    }
  }

  return fullText.substring(start, end).trim();
};
