import { splitQueries } from './sql';

export interface OnlineDdlJob {
  id: string;
  connectionId: string;
  database: string;
  table: string;
  status: string;
  reason: string;
  progress: number | null;
  maxReplicaLagMs: number | null;
  finished: boolean;
  sql: string;
  resultDdl: string | null;
  logs: string[];
}

export interface OnlineDdlSettings {
  replicas: string;
  aliyunRds: boolean;
}

export function onlineDdlStatements(statements: string[]): string[] {
  const parsed = statements.flatMap(sql => splitQueries(sql, 'mysql'));
  if (parsed.length !== 1) throw new Error('Online DDL requires exactly one statement');
  return parsed;
}

export function replicaEndpoints(input: string): string[] {
  const values = input.split(/[\s,]+/).filter(Boolean);
  if (values.length === 0 || values.length > 16) throw new Error('Specify 1–16 direct replica endpoints');
  if (new Set(values).size !== values.length) throw new Error('Duplicate replica endpoint');
  for (const value of values) {
    if (/[/@?#]/.test(value)) throw new Error('Use replica host:port, not a URL');
    const url = new URL(`mysql://${value}`);
    if (!url.hostname || url.hostname.startsWith('-') || url.port === '0') throw new Error('Invalid replica endpoint');
  }
  return values;
}

export function loadOnlineDdlSettings(connectionId: string): OnlineDdlSettings {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(`online-ddl:${connectionId}`) || '{}');
    if (typeof stored === 'object' && stored !== null) {
      const value = stored as Record<string, unknown>;
      return { replicas: typeof value.replicas === 'string' ? value.replicas : '', aliyunRds: value.aliyunRds === true };
    }
  } catch (error) {
    console.warn('Online DDL settings could not be read; explicit replica selection is required', error);
  }
  return { replicas: '', aliyunRds: false };
}
