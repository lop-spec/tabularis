import { describe, it, expect } from 'vitest';
import {
  formatConnectionString,
  getDefaultPort,
  validateConnectionParams,
  getDriverLabel,
  hasSSH,
  generateConnectionName,
  type ConnectionParams,
  type DatabaseDriver,
} from '../../src/utils/connections';

describe('connections', () => {
  describe('formatConnectionString', () => {
    it('should format SQLite connection string', () => {
      const params: ConnectionParams = {
        driver: 'sqlite',
        database: '/path/to/database.db',
      };

      expect(formatConnectionString(params)).toBe('/path/to/database.db');
    });

    it('should format PostgreSQL connection string with defaults', () => {
      const params: ConnectionParams = {
        driver: 'postgres',
        database: 'mydb',
        host: 'localhost',
      };

      expect(formatConnectionString(params)).toBe('localhost:5432/mydb');
    });

    it('should format MySQL connection string with custom port', () => {
      const params: ConnectionParams = {
        driver: 'mysql',
        database: 'mydb',
        host: 'db.example.com',
        port: 3307,
      };

      expect(formatConnectionString(params)).toBe('db.example.com:3307/mydb');
    });

    it('should use localhost when host is not provided', () => {
      const params: ConnectionParams = {
        driver: 'postgres',
        database: 'mydb',
      };

      expect(formatConnectionString(params)).toBe('localhost:5432/mydb');
    });

    it('should format connection with IPv4 address', () => {
      const params: ConnectionParams = {
        driver: 'mysql',
        database: 'production',
        host: '192.168.1.100',
        port: 3306,
      };

      expect(formatConnectionString(params)).toBe('192.168.1.100:3306/production');
    });
  });

  describe('getDefaultPort', () => {
    it('should return correct default port for PostgreSQL', () => {
      expect(getDefaultPort('postgres')).toBe(5432);
    });

    it('should return correct default port for MySQL', () => {
      expect(getDefaultPort('mysql')).toBe(3306);
    });

    it('should return 0 for SQLite', () => {
      expect(getDefaultPort('sqlite')).toBe(0);
    });
  });

  describe('validateConnectionParams', () => {
    it('should validate a complete PostgreSQL connection', () => {
      const params: ConnectionParams = {
        driver: 'postgres',
        database: 'mydb',
        host: 'localhost',
        port: 5432,
        username: 'user',
        password: 'pass',
      };

      const result = validateConnectionParams(params);
      expect(result.isValid).toBe(true);
      expect(result.error).toBeUndefined();
    });

    it('should validate a complete SQLite connection', () => {
      const params: ConnectionParams = {
        driver: 'sqlite',
        database: '/path/to/db.sqlite',
      };

      const result = validateConnectionParams(params);
      expect(result.isValid).toBe(true);
    });

    it('should fail when driver is missing', () => {
      const params = {
        database: 'mydb',
      };

      const result = validateConnectionParams(params);
      expect(result.isValid).toBe(false);
      expect(result.error).toBe('Driver is required');
    });

    it('should fail when database is missing', () => {
      const params = {
        driver: 'postgres' as DatabaseDriver,
      };

      const result = validateConnectionParams(params);
      expect(result.isValid).toBe(false);
      expect(result.error).toBe('Database name is required');
    });

    it('should fail when host is missing for remote databases', () => {
      const params = {
        driver: 'mysql' as DatabaseDriver,
        database: 'mydb',
      };

      const result = validateConnectionParams(params);
      expect(result.isValid).toBe(false);
      expect(result.error).toBe('Host is required for remote databases');
    });

    it('should fail for invalid port numbers', () => {
      const params = {
        driver: 'postgres' as DatabaseDriver,
        database: 'mydb',
        host: 'localhost',
        port: 70000,
      };

      const result = validateConnectionParams(params);
      expect(result.isValid).toBe(false);
      expect(result.error).toBe('Port must be between 1 and 65535');
    });

    it('should fail for negative port numbers', () => {
      const params = {
        driver: 'postgres' as DatabaseDriver,
        database: 'mydb',
        host: 'localhost',
        port: -1,
      };

      const result = validateConnectionParams(params);
      expect(result.isValid).toBe(false);
    });

    it('should fail when SSH is enabled without host', () => {
      const params = {
        driver: 'postgres' as DatabaseDriver,
        database: 'mydb',
        host: 'localhost',
        ssh_enabled: true,
        ssh_user: 'user',
        ssh_password: 'pass',
      };

      // Missing ssh_host
      delete (params as { ssh_host?: string }).ssh_host;

      const result = validateConnectionParams(params);
      expect(result.isValid).toBe(false);
      expect(result.error).toBe('SSH host is required when SSH is enabled');
    });

    it('should fail when SSH is enabled without user', () => {
      const params = {
        driver: 'postgres' as DatabaseDriver,
        database: 'mydb',
        host: 'localhost',
        ssh_enabled: true,
        ssh_host: 'ssh.example.com',
        ssh_password: 'pass',
      };

      const result = validateConnectionParams(params);
      expect(result.isValid).toBe(false);
      expect(result.error).toBe('SSH user is required when SSH is enabled');
    });

    it('should fail when SSH is enabled without password or key', () => {
      const params = {
        driver: 'postgres' as DatabaseDriver,
        database: 'mydb',
        host: 'localhost',
        ssh_enabled: true,
        ssh_host: 'ssh.example.com',
        ssh_user: 'user',
      };

      const result = validateConnectionParams(params);
      expect(result.isValid).toBe(false);
      expect(result.error).toBe('SSH password or key file is required');
    });

    it('should validate SSH connection with password', () => {
      const params: ConnectionParams = {
        driver: 'postgres',
        database: 'mydb',
        host: 'localhost',
        ssh_enabled: true,
        ssh_host: 'ssh.example.com',
        ssh_user: 'user',
        ssh_password: 'pass',
      };

      const result = validateConnectionParams(params);
      expect(result.isValid).toBe(true);
    });

    it('should validate SSH connection with key file', () => {
      const params: ConnectionParams = {
        driver: 'postgres',
        database: 'mydb',
        host: 'localhost',
        ssh_enabled: true,
        ssh_host: 'ssh.example.com',
        ssh_user: 'user',
        ssh_key_file: '/path/to/key',
      };

      const result = validateConnectionParams(params);
      expect(result.isValid).toBe(true);
    });

    it('should fail for invalid SSH port', () => {
      const params = {
        driver: 'postgres' as DatabaseDriver,
        database: 'mydb',
        host: 'localhost',
        ssh_enabled: true,
        ssh_host: 'ssh.example.com',
        ssh_user: 'user',
        ssh_password: 'pass',
        ssh_port: 100000,
      };

      const result = validateConnectionParams(params);
      expect(result.isValid).toBe(false);
      expect(result.error).toBe('SSH port must be between 1 and 65535');
    });
  });

  describe('getDriverLabel', () => {
    it('should return human-readable label for PostgreSQL', () => {
      expect(getDriverLabel('postgres')).toBe('PostgreSQL');
    });

    it('should return human-readable label for MySQL', () => {
      expect(getDriverLabel('mysql')).toBe('MySQL');
    });

    it('should return human-readable label for SQLite', () => {
      expect(getDriverLabel('sqlite')).toBe('SQLite');
    });
  });

  describe('hasSSH', () => {
    it('should return true when SSH is enabled', () => {
      const params: ConnectionParams = {
        driver: 'postgres',
        database: 'mydb',
        host: 'localhost',
        ssh_enabled: true,
      };

      expect(hasSSH(params)).toBe(true);
    });

    it('should return false when SSH is disabled', () => {
      const params: ConnectionParams = {
        driver: 'postgres',
        database: 'mydb',
        host: 'localhost',
        ssh_enabled: false,
      };

      expect(hasSSH(params)).toBe(false);
    });

    it('should return false when SSH is undefined', () => {
      const params: ConnectionParams = {
        driver: 'postgres',
        database: 'mydb',
        host: 'localhost',
      };

      expect(hasSSH(params)).toBe(false);
    });
  });

  describe('generateConnectionName', () => {
    it('should generate name from SQLite path', () => {
      const params: ConnectionParams = {
        driver: 'sqlite',
        database: '/path/to/database.db',
      };

      expect(generateConnectionName(params)).toBe('database.db');
    });

    it('should generate name from remote database', () => {
      const params: ConnectionParams = {
        driver: 'postgres',
        database: 'production',
        host: 'db.example.com',
      };

      expect(generateConnectionName(params)).toBe('production@db.example.com');
    });

    it('should use localhost when host is not provided', () => {
      const params: ConnectionParams = {
        driver: 'mysql',
        database: 'development',
      };

      expect(generateConnectionName(params)).toBe('development@localhost');
    });

    it('should handle SQLite path with no slashes', () => {
      const params: ConnectionParams = {
        driver: 'sqlite',
        database: 'local.db',
      };

      expect(generateConnectionName(params)).toBe('local.db');
    });

    it('should extract filename from complex SQLite path', () => {
      const params: ConnectionParams = {
        driver: 'sqlite',
        database: '/home/user/projects/app/data/main.sqlite',
      };

      expect(generateConnectionName(params)).toBe('main.sqlite');
    });
  });
});
