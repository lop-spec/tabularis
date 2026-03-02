import { Network, Database, FolderOpen, Plug } from "lucide-react";
import type { ReactNode } from "react";
import type { PluginManifest } from "../types/plugins";

const FALLBACK_COLOR = "#64748b"; // slate-500

/**
 * Returns the hex color for a driver, falling back to a neutral gray.
 */
export function getDriverColor(manifest: PluginManifest | undefined | null): string {
  return manifest?.color || FALLBACK_COLOR;
}

/**
 * Returns a lucide icon ReactNode for a driver based on its manifest icon field.
 */
export function getDriverIcon(manifest: PluginManifest | undefined | null, size = 14): ReactNode {
  const iconName = manifest?.icon || "";
  switch (iconName) {
    case "network":
      return <Network size={size} />;
    case "database":
      return <Database size={size} />;
    case "folder-open":
      return <FolderOpen size={size} />;
    default:
      return <Plug size={size} />;
  }
}

/**
 * Returns an inline style object with backgroundColor set to the driver color.
 * Use this on elements that render a colored driver badge/dot.
 */
export function getDriverColorStyle(manifest: PluginManifest | undefined | null): { backgroundColor: string } {
  return { backgroundColor: getDriverColor(manifest) };
}
