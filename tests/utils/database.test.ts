import { describe, it, expect } from 'vitest';
import {
  isMultiDatabaseCapable,
  isMultiDatabaseSelection,
  getTableDataChangeScope,
  getDatabaseList,
  getEffectiveDatabase,
  reconcileDatabaseSelection,
  resolveExecutionScope,
  resolveActiveDatabase,
  changesDatabaseCatalog,
  parseUseDatabaseStatement,
  resolveUseDatabaseSwitch,
} from '../../src/utils/database';
import type { DriverCapabilities } from '../../src/types/plugins';

const baseCapabilities: DriverCapabilities = {
  schemas: false,
  views: true,
  routines: true,
  file_based: false,
  folder_based: false,
  identifier_quote: '`',
  alter_primary_key: false,
};

describe('isMultiDatabaseCapable', () => {
  it('returns true for MySQL-like driver (no schemas, not file_based, not folder_based)', () => {
    expect(isMultiDatabaseCapable(baseCapabilities)).toBe(true);
  });

  it('returns false when schemas is true (Postgres)', () => {
    expect(isMultiDatabaseCapable({ ...baseCapabilities, schemas: true })).toBe(false);
  });

  it('returns false when file_based is true (SQLite)', () => {
    expect(isMultiDatabaseCapable({ ...baseCapabilities, file_based: true })).toBe(false);
  });

  it('returns false when folder_based is true (DuckDB)', () => {
    expect(isMultiDatabaseCapable({ ...baseCapabilities, folder_based: true })).toBe(false);
  });

  it('returns false for a single_database store (Meilisearch)', () => {
    expect(isMultiDatabaseCapable({ ...baseCapabilities, single_database: true })).toBe(false);
  });

  it('returns false when both schemas and file_based are true', () => {
    expect(isMultiDatabaseCapable({ ...baseCapabilities, schemas: true, file_based: true })).toBe(false);
  });

  it('returns false for null capabilities', () => {
    expect(isMultiDatabaseCapable(null)).toBe(false);
  });

  it('returns false for undefined capabilities', () => {
    expect(isMultiDatabaseCapable(undefined)).toBe(false);
  });
});

describe('getTableDataChangeScope', () => {
  it('prefers the table tab schema for schema-capable drivers', () => {
    expect(
      getTableDataChangeScope(
        { ...baseCapabilities, schemas: true },
        'schema_a',
        'schema_b',
      ),
    ).toEqual({ schema: 'schema_a' });
  });

  it('falls back to the active schema when a schema-capable tab has no schema', () => {
    expect(
      getTableDataChangeScope(
        { ...baseCapabilities, schemas: true },
        undefined,
        'public',
      ),
    ).toEqual({ schema: 'public' });
  });

  it('uses the table tab value as database for multi-database drivers', () => {
    expect(getTableDataChangeScope(baseCapabilities, 'app_db', 'ignored')).toEqual({
      database: 'app_db',
    });
  });

  it('omits scope for flat drivers', () => {
    expect(
      getTableDataChangeScope(
        { ...baseCapabilities, file_based: true },
        'main',
        'public',
      ),
    ).toEqual({});
  });
});

describe('isMultiDatabaseSelection', () => {
  it('returns true for an array', () => {
    expect(isMultiDatabaseSelection(['db1', 'db2'])).toBe(true);
  });

  it('returns true for an empty array', () => {
    expect(isMultiDatabaseSelection([])).toBe(true);
  });

  it('returns true for a single-element array', () => {
    expect(isMultiDatabaseSelection(['db1'])).toBe(true);
  });

  it('returns false for a string', () => {
    expect(isMultiDatabaseSelection('mydb')).toBe(false);
  });

  it('returns false for an empty string', () => {
    expect(isMultiDatabaseSelection('')).toBe(false);
  });
});

describe('getDatabaseList', () => {
  it('returns the array unchanged when given an array', () => {
    expect(getDatabaseList(['db1', 'db2'])).toEqual(['db1', 'db2']);
  });

  it('returns empty array for empty array input', () => {
    expect(getDatabaseList([])).toEqual([]);
  });

  it('wraps a non-empty string in an array', () => {
    expect(getDatabaseList('mydb')).toEqual(['mydb']);
  });

  it('returns empty array for an empty string', () => {
    expect(getDatabaseList('')).toEqual([]);
  });

  it('returns single-element array for single-element array input', () => {
    expect(getDatabaseList(['only'])).toEqual(['only']);
  });
});

describe('reconcileDatabaseSelection', () => {
  it('keeps the selection unchanged when every database exists', () => {
    expect(reconcileDatabaseSelection(['a', 'b'], ['a', 'b', 'c'])).toEqual({
      selection: ['a', 'b'],
      removed: [],
    });
  });

  it('removes databases that no longer exist on the server', () => {
    expect(reconcileDatabaseSelection(['a', 'vins', 'b'], ['a', 'b', 'c'])).toEqual({
      selection: ['a', 'b'],
      removed: ['vins'],
    });
  });

  it('preserves the saved order of the surviving selection', () => {
    expect(reconcileDatabaseSelection(['z', 'a', 'm'], ['a', 'm', 'z']).selection).toEqual([
      'z',
      'a',
      'm',
    ]);
  });

  it('reports everything as removed when the server list is empty', () => {
    expect(reconcileDatabaseSelection(['a', 'b'], [])).toEqual({
      selection: [],
      removed: ['a', 'b'],
    });
  });

  it('returns empty results for an empty saved selection', () => {
    expect(reconcileDatabaseSelection([], ['a', 'b'])).toEqual({
      selection: [],
      removed: [],
    });
  });

  it('matches database names case-sensitively', () => {
    expect(reconcileDatabaseSelection(['Vins'], ['vins'])).toEqual({
      selection: [],
      removed: ['Vins'],
    });
  });

  it('keeps duplicate saved entries that exist on the server', () => {
    expect(reconcileDatabaseSelection(['a', 'a'], ['a'])).toEqual({
      selection: ['a', 'a'],
      removed: [],
    });
  });
});

describe('getEffectiveDatabase', () => {
  it('returns the string as-is', () => {
    expect(getEffectiveDatabase('mydb')).toBe('mydb');
  });

  it('returns empty string for empty string input', () => {
    expect(getEffectiveDatabase('')).toBe('');
  });

  it('returns the first element of an array', () => {
    expect(getEffectiveDatabase(['db1', 'db2', 'db3'])).toBe('db1');
  });

  it('returns empty string for empty array', () => {
    expect(getEffectiveDatabase([])).toBe('');
  });

  it('returns the only element of a single-element array', () => {
    expect(getEffectiveDatabase(['only'])).toBe('only');
  });
});

describe('resolveExecutionScope', () => {
  it('keeps an explicitly pinned tab database ahead of global state', () => {
    expect(
      resolveExecutionScope('right_click_db', 'default_db', ['default_db', 'right_click_db'], true),
    ).toBe('right_click_db');
  });

  it('uses the visible first database when a multi-database tab has no scope yet', () => {
    expect(
      resolveExecutionScope(undefined, null, ['visible_db', 'other_db'], true),
    ).toBe('visible_db');
  });

  it('returns no scope instead of silently using a connection default', () => {
    expect(resolveExecutionScope(undefined, null, [], true)).toBeUndefined();
  });

  it('keeps the active PostgreSQL schema for schema-capable drivers', () => {
    expect(resolveExecutionScope(undefined, 'analytics', [], false)).toBe('analytics');
  });
});

describe('resolveActiveDatabase', () => {
  it('keeps the current database when it remains selected', () => {
    expect(resolveActiveDatabase(['app', 'audit'], 'audit')).toBe('audit');
  });

  it('falls back to the first remaining database after the active one disappears', () => {
    expect(resolveActiveDatabase(['audit', 'archive'], 'dropped_db')).toBe('audit');
  });

  it('returns null when no database remains', () => {
    expect(resolveActiveDatabase([], 'dropped_db')).toBeNull();
  });
});

describe('changesDatabaseCatalog', () => {
  it('detects database catalog DDL', () => {
    expect(changesDatabaseCatalog('DROP DATABASE archive')).toBe(true);
    expect(changesDatabaseCatalog('CREATE SCHEMA analytics')).toBe(true);
  });

  it('does not classify ordinary table DDL as a database catalog change', () => {
    expect(changesDatabaseCatalog('DROP TABLE archive')).toBe(false);
  });
});

describe('parseUseDatabaseStatement', () => {
  it('parses plain and quoted MySQL database identifiers', () => {
    expect(parseUseDatabaseStatement('USE analytics')).toBe('analytics');
    expect(parseUseDatabaseStatement('use `order-db`;')).toBe('order-db');
    expect(parseUseDatabaseStatement('USE `odd``name`')).toBe('odd`name');
  });

  it('accepts leading SQL comments but rejects non-USE statements and trailing SQL', () => {
    expect(parseUseDatabaseStatement('/* target */\n-- switch now\nUSE archive;')).toBe('archive');
    expect(parseUseDatabaseStatement("SELECT 'USE archive'")).toBeNull();
    expect(parseUseDatabaseStatement('USE archive SELECT 1')).toBeNull();
  });
});

describe('resolveUseDatabaseSwitch', () => {
  it('reports success without switching when the console already uses the database', () => {
    expect(
      resolveUseDatabaseSwitch('USE app', 'app', ['app', 'audit']),
    ).toEqual({
      database: 'app',
      shouldSwitch: false,
      shouldAddToSelection: false,
    });
  });

  it('switches only the current console scope and keeps the database selectable', () => {
    expect(
      resolveUseDatabaseSwitch('USE `audit`', 'app', ['app']),
    ).toEqual({
      database: 'audit',
      shouldSwitch: true,
      shouldAddToSelection: true,
    });
  });
});
