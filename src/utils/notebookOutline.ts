import type { NotebookCell } from "../types/notebook";

export interface OutlineEntry {
  level: number;
  text: string;
  cellId: string;
  cellIndex: number;
}

const HEADING_REGEX = /^(#{1,3})\s+(.+)$/;

/** Extract heading entries from markdown cells for TOC rendering. */
export function extractOutline(cells: NotebookCell[]): OutlineEntry[] {
  const entries: OutlineEntry[] = [];

  for (let i = 0; i < cells.length; i++) {
    const cell = cells[i];
    if (cell.type !== "markdown" || !cell.content.trim()) continue;

    const lines = cell.content.split("\n");
    for (const line of lines) {
      const match = line.match(HEADING_REGEX);
      if (match) {
        entries.push({
          level: match[1].length,
          text: match[2].trim(),
          cellId: cell.id,
          cellIndex: i,
        });
      }
    }
  }

  return entries;
}
