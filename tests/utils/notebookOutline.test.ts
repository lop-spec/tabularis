import { describe, it, expect } from "vitest";
import { extractOutline } from "../../src/utils/notebookOutline";
import type { NotebookCell } from "../../src/types/notebook";

function makeCell(overrides: Partial<NotebookCell> = {}): NotebookCell {
  return {
    id: "cell-1",
    type: "markdown",
    content: "",
    ...overrides,
  };
}

describe("notebookOutline", () => {
  describe("extractOutline", () => {
    it("extracts headings from markdown cells", () => {
      const cells = [
        makeCell({ id: "c1", content: "# Introduction\nSome text\n## Setup" }),
        makeCell({ id: "c2", type: "sql", content: "SELECT 1" }),
        makeCell({ id: "c3", content: "## Analysis\n### Details" }),
      ];

      const entries = extractOutline(cells);
      expect(entries).toEqual([
        { level: 1, text: "Introduction", cellId: "c1", cellIndex: 0 },
        { level: 2, text: "Setup", cellId: "c1", cellIndex: 0 },
        { level: 2, text: "Analysis", cellId: "c3", cellIndex: 2 },
        { level: 3, text: "Details", cellId: "c3", cellIndex: 2 },
      ]);
    });

    it("returns empty array when no markdown cells", () => {
      const cells = [
        makeCell({ id: "c1", type: "sql", content: "SELECT 1" }),
      ];
      expect(extractOutline(cells)).toEqual([]);
    });

    it("returns empty array when no headings", () => {
      const cells = [
        makeCell({ id: "c1", content: "Just some text\nNo headings here" }),
      ];
      expect(extractOutline(cells)).toEqual([]);
    });

    it("skips empty markdown cells", () => {
      const cells = [
        makeCell({ id: "c1", content: "" }),
        makeCell({ id: "c2", content: "   " }),
        makeCell({ id: "c3", content: "# Title" }),
      ];

      const entries = extractOutline(cells);
      expect(entries).toEqual([
        { level: 1, text: "Title", cellId: "c3", cellIndex: 2 },
      ]);
    });

    it("only extracts h1-h3 headings", () => {
      const cells = [
        makeCell({
          id: "c1",
          content: "# H1\n## H2\n### H3\n#### H4\n##### H5",
        }),
      ];

      const entries = extractOutline(cells);
      expect(entries).toHaveLength(3);
      expect(entries.map((e) => e.level)).toEqual([1, 2, 3]);
    });

    it("trims heading text", () => {
      const cells = [
        makeCell({ id: "c1", content: "#   Spaced Title   " }),
      ];

      const entries = extractOutline(cells);
      expect(entries[0].text).toBe("Spaced Title");
    });

    it("handles empty cells array", () => {
      expect(extractOutline([])).toEqual([]);
    });
  });
});
