import { describe, it, expect } from "vitest";
import {
  generateSectionId,
  createSection,
  toggleSection,
  renameSection,
  removeSection,
  clearCellSection,
  assignCellToSection,
  getCellsInSection,
  getUnsectionedCells,
  groupCellsBySections,
} from "../../src/utils/notebookSections";
import type { NotebookCell, NotebookSection } from "../../src/types/notebook";

function makeCell(
  id: string,
  sectionId?: string,
): NotebookCell {
  return {
    id,
    type: "sql",
    content: "",
    result: null,
    error: undefined,
    executionTime: null,
    isLoading: false,
    sectionId,
  };
}

function makeSection(
  id: string,
  title: string,
  collapsed = false,
): NotebookSection {
  return { id, title, collapsed };
}

describe("notebookSections", () => {
  describe("generateSectionId", () => {
    it("should start with section_ prefix", () => {
      expect(generateSectionId()).toMatch(/^section_/);
    });

    it("should generate unique ids", () => {
      const ids = new Set(
        Array.from({ length: 50 }, () => generateSectionId()),
      );
      expect(ids.size).toBe(50);
    });
  });

  describe("createSection", () => {
    it("should create section with defaults", () => {
      const section = createSection("My Section");
      expect(section.title).toBe("My Section");
      expect(section.collapsed).toBe(false);
      expect(section.id).toMatch(/^section_/);
    });
  });

  describe("toggleSection", () => {
    it("should toggle collapsed state", () => {
      const sections = [makeSection("s1", "A", false)];
      const toggled = toggleSection(sections, "s1");
      expect(toggled[0].collapsed).toBe(true);
    });

    it("should only toggle target section", () => {
      const sections = [
        makeSection("s1", "A", false),
        makeSection("s2", "B", true),
      ];
      const toggled = toggleSection(sections, "s1");
      expect(toggled[0].collapsed).toBe(true);
      expect(toggled[1].collapsed).toBe(true);
    });
  });

  describe("renameSection", () => {
    it("should rename target section", () => {
      const sections = [makeSection("s1", "Old")];
      const renamed = renameSection(sections, "s1", "New");
      expect(renamed[0].title).toBe("New");
    });
  });

  describe("removeSection", () => {
    it("should remove target section", () => {
      const sections = [makeSection("s1", "A"), makeSection("s2", "B")];
      const removed = removeSection(sections, "s1");
      expect(removed).toHaveLength(1);
      expect(removed[0].id).toBe("s2");
    });
  });

  describe("clearCellSection", () => {
    it("should clear sectionId from cells in that section", () => {
      const cells = [makeCell("c1", "s1"), makeCell("c2", "s2")];
      const cleared = clearCellSection(cells, "s1");
      expect(cleared[0].sectionId).toBeUndefined();
      expect(cleared[1].sectionId).toBe("s2");
    });
  });

  describe("assignCellToSection", () => {
    it("should assign cell to section", () => {
      const cells = [makeCell("c1")];
      const assigned = assignCellToSection(cells, "c1", "s1");
      expect(assigned[0].sectionId).toBe("s1");
    });

    it("should remove cell from section when undefined", () => {
      const cells = [makeCell("c1", "s1")];
      const assigned = assignCellToSection(cells, "c1", undefined);
      expect(assigned[0].sectionId).toBeUndefined();
    });
  });

  describe("getCellsInSection", () => {
    it("should return cells matching section", () => {
      const cells = [
        makeCell("c1", "s1"),
        makeCell("c2", "s2"),
        makeCell("c3", "s1"),
      ];
      expect(getCellsInSection(cells, "s1")).toHaveLength(2);
    });
  });

  describe("getUnsectionedCells", () => {
    it("should return cells without section", () => {
      const cells = [makeCell("c1"), makeCell("c2", "s1"), makeCell("c3")];
      expect(getUnsectionedCells(cells)).toHaveLength(2);
    });
  });

  describe("groupCellsBySections", () => {
    it("should group consecutive cells with same section", () => {
      const sections = [makeSection("s1", "Section A")];
      const cells = [
        makeCell("c1", "s1"),
        makeCell("c2", "s1"),
        makeCell("c3"),
      ];
      const groups = groupCellsBySections(cells, sections);
      expect(groups).toHaveLength(2);
      expect(groups[0].type).toBe("section");
      if (groups[0].type === "section") {
        expect(groups[0].cells).toHaveLength(2);
        expect(groups[0].section.title).toBe("Section A");
        expect(groups[0].startIndex).toBe(0);
      }
      expect(groups[1].type).toBe("cell");
    });

    it("should handle all unsectioned cells", () => {
      const cells = [makeCell("c1"), makeCell("c2")];
      const groups = groupCellsBySections(cells, []);
      expect(groups).toHaveLength(2);
      expect(groups.every((g) => g.type === "cell")).toBe(true);
    });

    it("should handle interleaved sections", () => {
      const sections = [
        makeSection("s1", "A"),
        makeSection("s2", "B"),
      ];
      const cells = [
        makeCell("c1", "s1"),
        makeCell("c2"),
        makeCell("c3", "s2"),
      ];
      const groups = groupCellsBySections(cells, sections);
      expect(groups).toHaveLength(3);
      expect(groups[0].type).toBe("section");
      expect(groups[1].type).toBe("cell");
      expect(groups[2].type).toBe("section");
    });

    it("should ignore cells with non-existent section ids", () => {
      const cells = [makeCell("c1", "nonexistent")];
      const groups = groupCellsBySections(cells, []);
      expect(groups).toHaveLength(1);
      expect(groups[0].type).toBe("cell");
    });
  });
});
