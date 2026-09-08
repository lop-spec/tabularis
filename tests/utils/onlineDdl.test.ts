import { beforeEach, describe, expect, it, vi } from 'vitest';
import { loadOnlineDdlSettings, onlineDdlStatements, replicaEndpoints } from '../../src/utils/onlineDdl';

describe('onlineDdl', () => {
  beforeEach(() => localStorage.clear());
  it('uses the official MySQL splitter and rejects multiple statements', () => {
    expect(onlineDdlStatements(['ALTER TABLE `orders` ADD COLUMN `note` VARCHAR(32);'])).toHaveLength(1);
    expect(() => onlineDdlStatements(['ALTER TABLE `orders` ADD COLUMN x INT; DROP TABLE `orders`;'])).toThrow('exactly one');
    expect(() => onlineDdlStatements([])).toThrow('exactly one');
    expect(onlineDdlStatements(["ALTER TABLE `orders` ADD COLUMN x VARCHAR(32) DEFAULT 'a;b';"])).toHaveLength(1);
  });
  it('requires an explicit replica list without proxy URLs', () => {
    expect(replicaEndpoints('a.example.invalid:3306,\nb.example.invalid:3307')).toEqual(['a.example.invalid:3306', 'b.example.invalid:3307']);
    for (const input of ['', 'mysql://a.example.invalid', 'a.example.invalid a.example.invalid', 'user@a.example.invalid', 'a.example.invalid:0']) {
      expect(() => replicaEndpoints(input)).toThrow();
    }
  });
  it('persists only non-secret endpoint preferences per connection', () => {
    localStorage.setItem('online-ddl:fixture', JSON.stringify({ replicas: 'a.example.invalid', aliyunRds: true, password: 'not-a-setting' }));
    expect(loadOnlineDdlSettings('fixture')).toEqual({ replicas: 'a.example.invalid', aliyunRds: true });
    expect(loadOnlineDdlSettings('other')).toEqual({ replicas: '', aliyunRds: false });
  });
  it('logs a corrupt preference and requires fresh replica selection', () => {
    const warning = vi.spyOn(console, 'warn').mockImplementation(() => {});
    localStorage.setItem('online-ddl:fixture', '{');
    expect(loadOnlineDdlSettings('fixture').replicas).toBe('');
    expect(warning).toHaveBeenCalledOnce();
    warning.mockRestore();
  });
});
