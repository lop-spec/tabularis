import type { NotebookCell, NotebookSection } from "../types/notebook";

export function generateSectionId(): string {
  return `section_${Math.random().toString(36).substring(2, 9)}`;
}

export function createSection(title: string): NotebookSection {
  return {
    id: generateSectionId(),
    title,
    collapsed: false,
  };
}

export function toggleSection(
  sections: NotebookSection[],
  sectionId: string,
): NotebookSection[] {
  return sections.map((s) =>
    s.id === sectionId ? { ...s, collapsed: !s.collapsed } : s,
  );
}

export function renameSection(
  sections: NotebookSection[],
  sectionId: string,
  title: string,
): NotebookSection[] {
  return sections.map((s) =>
    s.id === sectionId ? { ...s, title } : s,
  );
}

export function removeSection(
  sections: NotebookSection[],
  sectionId: string,
): NotebookSection[] {
  return sections.filter((s) => s.id !== sectionId);
}

export function clearCellSection(
  cells: NotebookCell[],
  sectionId: string,
): NotebookCell[] {
  return cells.map((c) =>
    c.sectionId === sectionId ? { ...c, sectionId: undefined } : c,
  );
}

export function assignCellToSection(
  cells: NotebookCell[],
  cellId: string,
  sectionId: string | undefined,
): NotebookCell[] {
  return cells.map((c) =>
    c.id === cellId ? { ...c, sectionId } : c,
  );
}

export function getCellsInSection(
  cells: NotebookCell[],
  sectionId: string,
): NotebookCell[] {
  return cells.filter((c) => c.sectionId === sectionId);
}

export function getUnsectionedCells(cells: NotebookCell[]): NotebookCell[] {
  return cells.filter((c) => !c.sectionId);
}

export interface GroupedCells {
  type: "section";
  section: NotebookSection;
  cells: NotebookCell[];
  startIndex: number;
}

export interface UngroupedCell {
  type: "cell";
  cell: NotebookCell;
  index: number;
}

export type CellGroup = GroupedCells | UngroupedCell;

export function groupCellsBySections(
  cells: NotebookCell[],
  sections: NotebookSection[],
): CellGroup[] {
  const sectionMap = new Map(sections.map((s) => [s.id, s]));
  const groups: CellGroup[] = [];
  let currentSectionId: string | undefined;
  let currentGroup: GroupedCells | null = null;

  cells.forEach((cell, index) => {
    if (cell.sectionId && sectionMap.has(cell.sectionId)) {
      if (cell.sectionId !== currentSectionId) {
        if (currentGroup) groups.push(currentGroup);
        currentSectionId = cell.sectionId;
        currentGroup = {
          type: "section",
          section: sectionMap.get(cell.sectionId)!,
          cells: [cell],
          startIndex: index,
        };
      } else {
        currentGroup!.cells.push(cell);
      }
    } else {
      if (currentGroup) {
        groups.push(currentGroup);
        currentGroup = null;
        currentSectionId = undefined;
      }
      groups.push({ type: "cell", cell, index });
    }
  });

  if (currentGroup) groups.push(currentGroup);
  return groups;
}
