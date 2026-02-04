/**
 * Tests for SSH connection utilities
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { formatSshConnectionString, validateSshConnection, testSshConnection } from "./ssh";
import type { SshConnection } from "./ssh";

// Mock Tauri's invoke
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("SSH Utilities", () => {
  describe("formatSshConnectionString", () => {
    it("should format SSH connection string correctly", () => {
      const ssh: SshConnection = {
        id: "1",
        name: "Test SSH",
        host: "example.com",
        port: 22,
        user: "testuser",
      };

      const result = formatSshConnectionString(ssh);
      expect(result).toBe("testuser@example.com:22");
    });

    it("should handle custom ports", () => {
      const ssh: SshConnection = {
        id: "1",
        name: "Test SSH",
        host: "example.com",
        port: 2222,
        user: "testuser",
      };

      const result = formatSshConnectionString(ssh);
      expect(result).toBe("testuser@example.com:2222");
    });
  });

  describe("validateSshConnection", () => {
    it("should validate a complete SSH connection", () => {
      const ssh: Partial<SshConnection> = {
        name: "Test",
        host: "example.com",
        port: 22,
        user: "testuser",
        password: "secret",
      };

      const result = validateSshConnection(ssh);
      expect(result.isValid).toBe(true);
      expect(result.error).toBeUndefined();
    });

    it("should require a name", () => {
      const ssh: Partial<SshConnection> = {
        host: "example.com",
        user: "testuser",
        password: "secret",
      };

      const result = validateSshConnection(ssh);
      expect(result.isValid).toBe(false);
      expect(result.error).toBe("Connection name is required");
    });

    it("should require a host", () => {
      const ssh: Partial<SshConnection> = {
        name: "Test",
        user: "testuser",
        password: "secret",
      };

      const result = validateSshConnection(ssh);
      expect(result.isValid).toBe(false);
      expect(result.error).toBe("SSH host is required");
    });

    it("should require a user", () => {
      const ssh: Partial<SshConnection> = {
        name: "Test",
        host: "example.com",
        password: "secret",
      };

      const result = validateSshConnection(ssh);
      expect(result.isValid).toBe(false);
      expect(result.error).toBe("SSH user is required");
    });

    it("should validate port range", () => {
      const ssh: Partial<SshConnection> = {
        name: "Test",
        host: "example.com",
        user: "testuser",
        password: "secret",
        port: 70000,
      };

      const result = validateSshConnection(ssh);
      expect(result.isValid).toBe(false);
      expect(result.error).toBe("SSH port must be between 1 and 65535");
    });

    it("should require password or key file", () => {
      const ssh: Partial<SshConnection> = {
        name: "Test",
        host: "example.com",
        user: "testuser",
      };

      const result = validateSshConnection(ssh);
      expect(result.isValid).toBe(false);
      expect(result.error).toBe("SSH password or key file is required");
    });

    it("should accept key file instead of password", () => {
      const ssh: Partial<SshConnection> = {
        name: "Test",
        host: "example.com",
        user: "testuser",
        key_file: "/path/to/key",
      };

      const result = validateSshConnection(ssh);
      expect(result.isValid).toBe(true);
    });
  });

  describe("testSshConnection", () => {
    beforeEach(() => {
      vi.clearAllMocks();
    });

    it("should call invoke with correct parameters", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      vi.mocked(invoke).mockResolvedValue("SSH connection successful");

      const ssh: Partial<SshConnection> = {
        host: "example.com",
        port: 22,
        user: "testuser",
        password: "secret",
      };

      const result = await testSshConnection(ssh);

      expect(invoke).toHaveBeenCalledWith("test_ssh_connection", {
        ssh: expect.objectContaining({
          host: "example.com",
          port: 22,
          user: "testuser",
          password: "secret",
        }),
      });
      expect(result).toBe("SSH connection successful");
    });

    it("should normalize empty password to undefined", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      vi.mocked(invoke).mockResolvedValue("SSH connection successful");

      const ssh: Partial<SshConnection> = {
        host: "example.com",
        port: 22,
        user: "testuser",
        password: "  ", // Empty/whitespace password
      };

      await testSshConnection(ssh);

      expect(invoke).toHaveBeenCalledWith("test_ssh_connection", {
        ssh: expect.objectContaining({
          password: undefined, // Should be normalized
        }),
      });
    });

    it("should propagate errors from invoke", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      const errorMessage = "Connection refused";
      vi.mocked(invoke).mockRejectedValue(new Error(errorMessage));

      const ssh: Partial<SshConnection> = {
        host: "example.com",
        port: 22,
        user: "testuser",
        password: "secret",
      };

      await expect(testSshConnection(ssh)).rejects.toThrow(errorMessage);
    });
  });
});
