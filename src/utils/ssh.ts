/**
 * SSH connection utilities
 * Extracted for testability and reusability
 */

import { invoke } from "@tauri-apps/api/core";

export interface SshConnection {
  id: string;
  name: string;
  host: string;
  port: number;
  user: string;
  auth_type?: "password" | "ssh_key";
  password?: string;
  key_file?: string;
  key_passphrase?: string;
  save_in_keychain?: boolean;
}

/**
 * Load all SSH connections
 */
export async function loadSshConnections(): Promise<SshConnection[]> {
  try {
    return await invoke<SshConnection[]>("get_ssh_connections");
  } catch (error) {
    console.error("Failed to load SSH connections:", error);
    return [];
  }
}

/**
 * Normalize SSH params: only normalize key_file, preserve password/passphrase intent
 */
function normalizeSshParams(ssh: Partial<SshConnection>): Partial<SshConnection> {
  const result: Partial<SshConnection> = { ...ssh };

  // Only normalize key_file (empty strings become undefined)
  if (ssh.key_file !== undefined && !ssh.key_file?.trim()) {
    delete result.key_file;
  }

  // Don't normalize password/key_passphrase - preserve caller's intent
  // If caller sends empty string explicitly, it means "use empty password"
  // If caller omits the field, it means "use keychain"

  return result;
}

/**
 * Save a new SSH connection
 */
export async function saveSshConnection(
  name: string,
  ssh: Partial<SshConnection>
): Promise<SshConnection> {
  return await invoke<SshConnection>("save_ssh_connection", {
    name,
    ssh: normalizeSshParams(ssh)
  });
}

/**
 * Update an existing SSH connection
 */
export async function updateSshConnection(
  id: string,
  name: string,
  ssh: Partial<SshConnection>
): Promise<SshConnection> {
  return await invoke<SshConnection>("update_ssh_connection", {
    id,
    name,
    ssh: normalizeSshParams(ssh)
  });
}

/**
 * Delete an SSH connection
 */
export async function deleteSshConnection(id: string): Promise<void> {
  await invoke("delete_ssh_connection", { id });
}

/**
 * Test an SSH connection
 * @returns Success message if connection works
 * @throws Error with message if connection fails
 */
export async function testSshConnection(
  ssh: Partial<SshConnection>
): Promise<string> {
  return await invoke<string>("test_ssh_connection", {
    ssh: {
      ...normalizeSshParams(ssh),
      connection_id: ssh.id
    }
  });
}

/**
 * Format an SSH connection for display
 */
export function formatSshConnectionString(ssh: SshConnection): string {
  return `${ssh.user}@${ssh.host}:${ssh.port}`;
}

/**
 * Validate SSH connection parameters
 */
export interface SshValidationResult {
  isValid: boolean;
  error?: string;
}

export function validateSshConnection(
  ssh: Partial<SshConnection>,
  options: { allowEmptyPassword?: boolean } = {}
): SshValidationResult {
  const { allowEmptyPassword = false } = options;
  if (!ssh.name || ssh.name.trim() === "") {
    return { isValid: false, error: "Connection name is required" };
  }

  if (!ssh.host || ssh.host.trim() === "") {
    return { isValid: false, error: "SSH host is required" };
  }

  if (!ssh.user || ssh.user.trim() === "") {
    return { isValid: false, error: "SSH user is required" };
  }

  if (ssh.port !== undefined && (ssh.port < 1 || ssh.port > 65535)) {
    return { isValid: false, error: "SSH port must be between 1 and 65535" };
  }

  if (!ssh.auth_type) {
    return { isValid: false, error: "Authentication type is required" };
  }

  // Validate based on auth type
  if (ssh.auth_type === "password" && !allowEmptyPassword) {
    if (!ssh.password || ssh.password.trim() === "") {
      return { isValid: false, error: "Password is required for password authentication" };
    }
  }
  // For ssh_key type, both key_file and key_passphrase are optional

  return { isValid: true };
}
