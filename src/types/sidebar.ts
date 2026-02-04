import type { SavedQuery } from "../contexts/SavedQueriesContext";

export type ContextMenuData = SavedQuery | { tableName: string };
