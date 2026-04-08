import { describe, it, expect } from "vitest";
import { reorderCells, getDropIndex } from "../../src/utils/notebookDnd";
import type { NotebookCell } from "../../src/types/notebook";

function makeCell(id: string): NotebookCell {
  return {
    id,
    type: "sql",
    content: "",
    result: null,
    error: undefined,
    executionTime: null,
    isLoading: false,
  };
}

describe("notebookDnd", () => {
  describe("reorderCells", () => {
    it("should move cell from start to end", () => {
      const cells = [makeCell("a"), makeCell("b"), makeCell("c")];
      const result = reorderCells(cells, 0, 2);
      expect(result.map((c) => c.id)).toEqual(["b", "c", "a"]);
    });

    it("should move cell from end to start", () => {
      const cells = [makeCell("a"), makeCell("b"), makeCell("c")];
      const result = reorderCells(cells, 2, 0);
      expect(result.map((c) => c.id)).toEqual(["c", "a", "b"]);
    });

    it("should move cell to adjacent position", () => {
      const cells = [makeCell("a"), makeCell("b"), makeCell("c")];
      const result = reorderCells(cells, 0, 1);
      expect(result.map((c) => c.id)).toEqual(["b", "a", "c"]);
    });

    it("should no-op when from equals to", () => {
      const cells = [makeCell("a"), makeCell("b")];
      const result = reorderCells(cells, 0, 0);
      expect(result.map((c) => c.id)).toEqual(["a", "b"]);
    });

    it("should no-op for out-of-bounds from index", () => {
      const cells = [makeCell("a"), makeCell("b")];
      expect(reorderCells(cells, -1, 0)).toEqual(cells);
      expect(reorderCells(cells, 5, 0)).toEqual(cells);
    });

    it("should no-op for out-of-bounds to index", () => {
      const cells = [makeCell("a"), makeCell("b")];
      expect(reorderCells(cells, 0, -1)).toEqual(cells);
      expect(reorderCells(cells, 0, 5)).toEqual(cells);
    });

    it("should not mutate original array", () => {
      const cells = [makeCell("a"), makeCell("b"), makeCell("c")];
      const original = [...cells];
      reorderCells(cells, 0, 2);
      expect(cells.map((c) => c.id)).toEqual(original.map((c) => c.id));
    });
  });

  describe("getDropIndex", () => {
    const containerRect = { top: 0, left: 0, width: 400, height: 300 } as DOMRect;

    function makeRect(top: number, height: number): DOMRect {
      return {
        top,
        bottom: top + height,
        left: 0,
        right: 400,
        width: 400,
        height,
        x: 0,
        y: top,
        toJSON: () => ({}),
      };
    }

    it("should return 0 when cursor is above first cell midpoint", () => {
      const rects = [makeRect(0, 100), makeRect(100, 100), makeRect(200, 100)];
      expect(getDropIndex(containerRect, rects, 30, 2)).toBe(0);
    });

    it("should return last index when cursor is below all cells", () => {
      const rects = [makeRect(0, 100), makeRect(100, 100), makeRect(200, 100)];
      expect(getDropIndex(containerRect, rects, 350, 0)).toBe(2);
    });

    it("should return correct index for middle position", () => {
      const rects = [makeRect(0, 100), makeRect(100, 100), makeRect(200, 100)];
      expect(getDropIndex(containerRect, rects, 130, 0)).toBe(1);
    });
  });
});
