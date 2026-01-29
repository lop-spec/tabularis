export function splitQueries(sql: string): string[] {
  const queries: string[] = [];
  let currentQuery = '';
  let inQuote = false;
  let quoteChar = '';
  let inLineComment = false; // --
  let inBlockComment = false; // /* */

  for (let i = 0; i < sql.length; i++) {
    const char = sql[i];
    const nextChar = sql[i + 1];

    if (inLineComment) {
        if (char === '\n') inLineComment = false;
        currentQuery += char;
    } else if (inBlockComment) {
        if (char === '*' && nextChar === '/') {
            inBlockComment = false;
            currentQuery += '*/';
            i++;
        } else {
            currentQuery += char;
        }
    } else if (inQuote) {
        if (char === quoteChar) {
            // Check for escape (double quote for SQL usually) - e.g. 'It''s'
            if (nextChar === quoteChar) {
                currentQuery += char + nextChar;
                i++;
            } else {
                inQuote = false;
                currentQuery += char;
            }
        } else {
            currentQuery += char;
        }
    } else {
        // Normal state
        if (char === '-' && nextChar === '-') {
            inLineComment = true;
            currentQuery += '--';
            i++;
        } else if (char === '/' && nextChar === '*') {
            inBlockComment = true;
            currentQuery += '/*';
            i++;
        } else if (char === "'" || char === '"' || char === '`') {
            inQuote = true;
            quoteChar = char;
            currentQuery += char;
        } else if (char === ';') {
            if (currentQuery.trim()) {
                queries.push(currentQuery.trim());
            }
            currentQuery = '';
        } else {
            currentQuery += char;
        }
    }
  }
  if (currentQuery.trim()) {
      queries.push(currentQuery.trim());
  }
  return queries;
}

/**
 * Extracts the table name from a SELECT query.
 * Handles quotes: `table`, "table", 'table', and unquoted table names.
 * Returns null if no table is found or if it's not a SELECT query.
 * Returns null for aggregate queries (COUNT, SUM, etc.) since they don't return table rows.
 */
export function extractTableName(sql: string): string | null {
  // Remove comments and normalize whitespace
  const cleaned = sql
    .replace(/--[^\n]*/g, '') // Remove line comments
    .replace(/\/\*[\s\S]*?\*\//g, '') // Remove block comments
    .replace(/\s+/g, ' ') // Normalize whitespace
    .trim();

  // Check if it's a SELECT query
  if (!/^\s*SELECT\s+/i.test(cleaned)) {
    return null;
  }

  // Check if it's an aggregate query (COUNT, SUM, AVG, MIN, MAX, GROUP BY)
  // These don't return table rows, so we shouldn't fetch PK
  if (/\b(COUNT|SUM|AVG|MIN|MAX)\s*\(/i.test(cleaned) || /\bGROUP\s+BY\b/i.test(cleaned)) {
    return null;
  }

  // Match FROM clause with optional quotes
  // Matches: FROM table, FROM `table`, FROM "table", FROM 'table'
  const fromMatch = cleaned.match(/\bFROM\s+([`"']?)(\w+)\1/i);
  
  if (fromMatch && fromMatch[2]) {
    return fromMatch[2];
  }

  return null;
}
