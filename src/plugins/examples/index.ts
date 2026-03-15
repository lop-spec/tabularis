import type { SlotContribution } from "../../types/pluginSlots";
import { jsonViewerContributions } from "./json-viewer";

/**
 * All built-in example plugin contributions.
 * These are registered automatically by the PluginSlotProvider.
 */
export const builtinPluginContributions: SlotContribution[] = [
  ...jsonViewerContributions,
];
