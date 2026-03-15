import type { SlotContribution } from "../../../types/pluginSlots";
import JsonPreview from "./ui/JsonPreview";

const PLUGIN_ID = "json-viewer";

export const jsonViewerContributions: SlotContribution[] = [
  {
    pluginId: PLUGIN_ID,
    slot: "row-editor-sidebar.field.after",
    component: JsonPreview,
    order: 50,
  },
  {
    pluginId: PLUGIN_ID,
    slot: "row-edit-modal.field.after",
    component: JsonPreview,
    order: 50,
  },
];
