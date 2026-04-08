import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import type { NotebookState } from "../../src/types/notebook";

// Mock Tauri invoke
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// Import after mock setup
import { invoke } from "@tauri-apps/api/core";
import {
  getNotebookState,
  setNotebookState,
  loadNotebook,
  createNotebook,
  createNotebookFromState,
  deleteNotebook,
  evictFromCache,
  flushAllPendingSaves,
  getNotebookTitle,
  setNotebookTitle,
  _resetForTesting,
} from "../../src/utils/notebookStore";

const mockedInvoke = vi.mocked(invoke);

function makeState(content = "SELECT 1"): NotebookState {
  return {
    cells: [{ id: "c1", type: "sql", content }],
  };
}

describe("notebookStore", () => {
  beforeEach(() => {
    _resetForTesting();
    vi.useFakeTimers();
    mockedInvoke.mockReset();
    mockedInvoke.mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe("cache operations", () => {
    it("getNotebookState returns undefined for unknown ID", () => {
      expect(getNotebookState("unknown-id")).toBeUndefined();
    });

    it("setNotebookState stores state in cache", () => {
      const state = makeState();
      setNotebookState("test-1", state);
      expect(getNotebookState("test-1")).toBe(state);
    });
  });

  describe("debounced save", () => {
    it("does not save immediately on setNotebookState", () => {
      setNotebookState("debounce-1", makeState());
      expect(mockedInvoke).not.toHaveBeenCalledWith(
        "save_notebook",
        expect.anything(),
      );
    });

    it("saves after debounce period", async () => {
      setNotebookTitle("debounce-2", "Test");
      setNotebookState("debounce-2", makeState());

      await vi.advanceTimersByTimeAsync(1500);

      expect(mockedInvoke).toHaveBeenCalledWith("save_notebook", {
        notebookId: "debounce-2",
        content: expect.any(String),
      });
    });

    it("resets timer on subsequent calls", async () => {
      setNotebookTitle("debounce-3", "Test");
      setNotebookState("debounce-3", makeState("SELECT 1"));

      await vi.advanceTimersByTimeAsync(1000);
      expect(mockedInvoke).not.toHaveBeenCalledWith(
        "save_notebook",
        expect.anything(),
      );

      // Second call resets the timer
      setNotebookState("debounce-3", makeState("SELECT 2"));

      await vi.advanceTimersByTimeAsync(1000);
      // Still not saved (only 1000ms since last call)
      expect(mockedInvoke).not.toHaveBeenCalledWith(
        "save_notebook",
        expect.anything(),
      );

      await vi.advanceTimersByTimeAsync(500);
      // Now 1500ms since last call — should save
      expect(mockedInvoke).toHaveBeenCalledWith("save_notebook", {
        notebookId: "debounce-3",
        content: expect.any(String),
      });
    });
  });

  describe("loadNotebook", () => {
    it("loads from Tauri backend and caches", async () => {
      const fileContent = JSON.stringify({
        version: 2,
        title: "Loaded",
        createdAt: "2026-01-01",
        cells: [{ type: "sql", content: "SELECT 42" }],
      });
      mockedInvoke.mockResolvedValueOnce(fileContent);

      const state = await loadNotebook("load-1");

      expect(mockedInvoke).toHaveBeenCalledWith("load_notebook", {
        notebookId: "load-1",
      });
      expect(state.cells).toHaveLength(1);
      expect(state.cells[0].content).toBe("SELECT 42");
      expect(getNotebookState("load-1")).toBe(state);
      expect(getNotebookTitle("load-1")).toBe("Loaded");
    });

    it("returns cached state on second call", async () => {
      const state = makeState();
      setNotebookState("load-2", state);

      const result = await loadNotebook("load-2");
      expect(result).toBe(state);
      // Should NOT call invoke since it's cached
      expect(mockedInvoke).not.toHaveBeenCalledWith(
        "load_notebook",
        expect.anything(),
      );
    });

    it("creates default state when file not found", async () => {
      mockedInvoke.mockResolvedValueOnce(null);

      const state = await loadNotebook("load-3");
      expect(state.cells).toHaveLength(1);
      expect(state.cells[0].type).toBe("sql");
      expect(state.cells[0].content).toBe("");
    });
  });

  describe("createNotebook", () => {
    it("generates unique ID and saves to disk", async () => {
      const { notebookId, state } = await createNotebook("New Notebook");

      expect(notebookId).toMatch(/^nb_/);
      expect(state.cells).toHaveLength(1);
      expect(getNotebookState(notebookId)).toBe(state);
      expect(getNotebookTitle(notebookId)).toBe("New Notebook");
      expect(mockedInvoke).toHaveBeenCalledWith("create_notebook", {
        notebookId,
        content: expect.any(String),
      });
    });
  });

  describe("createNotebookFromState", () => {
    it("saves existing state as new file", async () => {
      const state = makeState("SELECT * FROM orders");
      const { notebookId } = await createNotebookFromState("Migrated", state);

      expect(notebookId).toMatch(/^nb_/);
      expect(getNotebookState(notebookId)).toBe(state);
      expect(getNotebookTitle(notebookId)).toBe("Migrated");
      expect(mockedInvoke).toHaveBeenCalledWith("create_notebook", {
        notebookId,
        content: expect.stringContaining("SELECT * FROM orders"),
      });
    });
  });

  describe("deleteNotebook", () => {
    it("evicts from cache and deletes file", async () => {
      setNotebookState("del-1", makeState());
      setNotebookTitle("del-1", "To Delete");

      await deleteNotebook("del-1");

      expect(getNotebookState("del-1")).toBeUndefined();
      expect(getNotebookTitle("del-1")).toBeUndefined();
      expect(mockedInvoke).toHaveBeenCalledWith("delete_notebook", {
        notebookId: "del-1",
      });
    });
  });

  describe("evictFromCache", () => {
    it("flushes pending save before evicting", async () => {
      setNotebookTitle("evict-1", "Test");
      setNotebookState("evict-1", makeState());

      // There's a pending save timer
      await evictFromCache("evict-1");

      // Should have flushed the save
      expect(mockedInvoke).toHaveBeenCalledWith("save_notebook", {
        notebookId: "evict-1",
        content: expect.any(String),
      });
      expect(getNotebookState("evict-1")).toBeUndefined();
    });
  });

  describe("flushAllPendingSaves", () => {
    it("flushes all pending timers", async () => {
      setNotebookTitle("flush-1", "A");
      setNotebookTitle("flush-2", "B");
      setNotebookState("flush-1", makeState("SELECT 1"));
      setNotebookState("flush-2", makeState("SELECT 2"));

      await flushAllPendingSaves();

      const saveCalls = mockedInvoke.mock.calls.filter(
        ([cmd]) => cmd === "save_notebook",
      );
      expect(saveCalls).toHaveLength(2);
    });
  });

  describe("title management", () => {
    it("stores and retrieves titles", () => {
      setNotebookTitle("title-1", "My Title");
      expect(getNotebookTitle("title-1")).toBe("My Title");
    });

    it("schedules save on title change", async () => {
      setNotebookState("title-2", makeState());
      setNotebookTitle("title-2", "Updated Title");

      await vi.advanceTimersByTimeAsync(1500);

      expect(mockedInvoke).toHaveBeenCalledWith("save_notebook", {
        notebookId: "title-2",
        content: expect.stringContaining("Updated Title"),
      });
    });
  });
});
