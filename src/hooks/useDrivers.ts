import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";

import type { PluginManifest } from "../types/plugins";

// MongoDB / Redis 的内置控制台已移除（2026-08-18）。后端源码里的注册也删了，
// 但正在用的 exe 是删除之前构建的，它仍以 is_builtin:true 注册这些 manifest，
// 会被下面那个 is_builtin 过滤放行。为此单独重编整棵 Rust 树不划算，
// 这里按 id 剔除即可；等 exe 因别的原因重建后，这个兜底自然失效但无害。
const RETIRED_DRIVER_IDS = new Set(["mongodb", "redis", "redis-rust", "redis-go"]);

const FALLBACK_DRIVERS: PluginManifest[] = [
  {
    id: "postgres",
    name: "PostgreSQL",
    version: "1.0.0",
    description: "PostgreSQL databases",
    default_port: 5432,
    is_builtin: true,
    default_username: "postgres",
    color: "#3b82f6",
    icon: "postgres",
    capabilities: {
      schemas: true,
      views: true,
      routines: true,
      file_based: false,
      folder_based: false,
      connection_string: true,
      connection_string_example: "postgres://user:pass@localhost:5432/db",
      identifier_quote: '"',
      alter_primary_key: true,
      auto_increment_keyword: "",
      serial_type: "SERIAL",
      inline_pk: false,
      alter_column: true,
      create_foreign_keys: true,
      supports_ssl: true,
      sql_dialect: "postgres",
    },
  },
  {
    id: "mysql",
    name: "MySQL",
    version: "1.0.0",
    description: "MySQL and MariaDB databases",
    default_port: 3306,
    is_builtin: true,
    default_username: "root",
    color: "#f97316",
    icon: "mysql",
    settings: [
      {
        key: "maxAllowedPacket",
        label: "Max Allowed Packet",
        type: "number",
        default: 1073741824,
        description: "Maximum packet size used by the MySQL connector.",
      },
      {
        key: "socketTimeout",
        label: "Socket Timeout",
        type: "number",
        default: 600000,
        description: "Socket timeout in milliseconds.",
      },
      {
        key: "connectTimeout",
        label: "Connect Timeout",
        type: "number",
        default: 60000,
        description: "Connection timeout in milliseconds.",
      },
      {
        key: "timezone",
        label: "Timezone",
        type: "string",
        default: "SYSTEM",
        description: "Session timezone sent to MySQL after connect.",
      },
    ],
    capabilities: {
      schemas: false,
      views: true,
      routines: true,
      file_based: false,
      folder_based: false,
      connection_string: true,
      connection_string_example: "mysql://user:pass@localhost:3306/db",
      identifier_quote: "`",
      alter_primary_key: true,
      auto_increment_keyword: "AUTO_INCREMENT",
      serial_type: "",
      inline_pk: false,
      alter_column: true,
      create_foreign_keys: true,
      supports_ssl: true,
      sql_dialect: "mysql",
    },
  },
  {
    id: "sqlite",
    name: "SQLite",
    version: "1.0.0",
    description: "SQLite file-based databases",
    default_port: null,
    is_builtin: true,
    default_username: "",
    color: "#06b6d4",
    icon: "sqlite",
    capabilities: {
      schemas: false,
      views: true,
      routines: false,
      file_based: true,
      folder_based: false,
      connection_string: false,
      identifier_quote: '"',
      alter_primary_key: true,
      auto_increment_keyword: "AUTOINCREMENT",
      serial_type: "",
      inline_pk: true,
      alter_column: false,
      create_foreign_keys: false,
      sql_dialect: "sqlite",
    },
  },
];

export function useDrivers(): {
  drivers: PluginManifest[];
  allDrivers: PluginManifest[];
  loading: boolean;
  error: string | null;
  refresh: () => void;
} {
  const [drivers, setDrivers] =
    useState<PluginManifest[]>(FALLBACK_DRIVERS);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    invoke<PluginManifest[]>("get_registered_drivers")
      .then((registeredDrivers) => {
        setDrivers(
          registeredDrivers.filter(
            (driver) => driver.is_builtin === true && !RETIRED_DRIVER_IDS.has(driver.id),
          ),
        );
        setError(null);
      })
      .catch((err: unknown) => {
        setError(String(err));
      })
      .finally(() => setLoading(false));
  }, []);

  const refresh = useCallback(() => {
    setLoading(true);
    load();
  }, [load]);

  useEffect(() => {
    load();
  }, [load]);

  return {
    drivers,
    allDrivers: drivers,
    loading,
    error,
    refresh,
  };
}
