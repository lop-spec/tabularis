import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { DatabaseProvider } from "../../src/contexts/DatabaseProvider";
import { useDatabase } from "../../src/hooks/useDatabase";
import { EditorProvider } from "../../src/contexts/EditorProvider";
import { useEditor } from "../../src/hooks/useEditor";
import type { SavedConnection } from "../../src/contexts/DatabaseContext";
import { promoteDefaultDatabase, getNewTabDatabase } from "../../src/utils/defaultDatabase";
import type { ReactNode } from "react";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }));
vi.mock("../../src/utils/autocomplete", () => ({ clearAutocompleteCache: vi.fn() }));

const wrapper = ({ children }: { children: ReactNode }) => <DatabaseProvider>{children}</DatabaseProvider>;
let saved: SavedConnection[];

beforeEach(() => {
  vi.clearAllMocks();
  saved = ["one", "two"].map((id) => ({
    id, name: id,
    params: { driver: "mysql", host: "db.example.invalid", database: ["alpha", "beta", "gamma"], username: "reader", rollback_protection_enabled: true },
  }));
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    switch (command) {
      case "get_connections": return structuredClone(saved);
      case "get_connections_with_groups": return { connections: structuredClone(saved), groups: [] };
      case "get_driver_manifest": return { capabilities: { file_based: false, schemas: false } };
      case "get_available_databases": return ["alpha", "beta", "gamma"];
      case "get_tables": case "get_views": case "get_routines": case "get_triggers": return [];
      case "load_editor_preferences": return null;
      case "set_selected_databases": {
        const input = args as { connectionId: string; databases: string[] };
        saved = saved.map((connection) => connection.id === input.connectionId
          ? { ...connection, params: { ...connection.params, database: input.databases } }
          : connection);
        return;
      }
      default: return;
    }
  });
});

describe("default database selection", () => {
  it("uses the new default only for new tabs and leaves explicit scopes and transactions untouched", async () => {
    const combined = ({ children }: { children: ReactNode }) =>
      <DatabaseProvider><EditorProvider>{children}</EditorProvider></DatabaseProvider>;
    const { result } = renderHook(() => ({ database: useDatabase(), editor: useEditor() }), { wrapper: combined });
    await act(async () => { await result.current.database.connect("one"); });
    await waitFor(() => expect(result.current.editor.tabs.length).toBeGreaterThan(0));
    let oldTab = "";
    act(() => { oldTab = result.current.editor.addTab({ type: "console", schema: "alpha", transactionActive: true }); });
    await act(async () => { await result.current.database.setDefaultDatabase("one", "beta"); });
    let newTab = "";
    act(() => { newTab = result.current.editor.addTab({ type: "console" }); });
    expect(result.current.editor.tabs.find((tab) => tab.id === newTab)?.schema).toBe("beta");
    expect(result.current.editor.tabs.find((tab) => tab.id === oldTab)).toMatchObject({ schema: "alpha", transactionActive: true });
    let explicitTab = "";
    act(() => { explicitTab = result.current.editor.addTab({ type: "console", schema: "gamma" }); });
    expect(result.current.editor.tabs.find((tab) => tab.id === explicitTab)?.schema).toBe("gamma");
    const data = result.current.database.connectionDataMap.one;
    expect(getNewTabDatabase({ ...data, capabilities: { ...data.capabilities!, schemas: true } })).toBeUndefined();
  });

  it("promotes only the requested exact name without mutating the original selection", () => {
    const selected = ["alpha", "Beta", "beta", "gamma"];
    expect(promoteDefaultDatabase(selected, "beta")).toEqual(["beta", "alpha", "Beta", "gamma"]);
    expect(selected).toEqual(["alpha", "Beta", "beta", "gamma"]);
    expect(() => promoteDefaultDatabase(selected, "missing")).toThrow();
    expect(() => promoteDefaultDatabase(selected, " ")).toThrow();
  });

  it("persists the default across reconnect and preserves current scope, other connections, and credentials", async () => {
    const { result } = renderHook(useDatabase, { wrapper });
    await act(async () => { await result.current.loadConnections(); await result.current.connect("one"); });
    const original = structuredClone(saved);
    await act(async () => { await result.current.setDefaultDatabase("one", "beta"); });
    expect(invoke).toHaveBeenCalledWith("set_selected_databases", { connectionId: "one", databases: ["beta", "alpha", "gamma"] });
    expect(result.current.selectedDatabases).toEqual(["beta", "alpha", "gamma"]);
    expect(result.current.activeDatabaseName).toBe("beta");
    expect(result.current.activeSchema).toBe("alpha");
    expect(getNewTabDatabase(result.current.connectionDataMap.one)).toBe("beta");
    expect(result.current.connections[0].params.database).toEqual(["beta", "alpha", "gamma"]);
    expect(saved[1]).toEqual(original[1]);
    expect({ ...saved[0].params, database: original[0].params.database }).toEqual(original[0].params);
    expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "update_connection" || command === "execute_query")).toBe(false);
    await act(async () => { await result.current.disconnect("one"); });
    await act(async () => { await result.current.connect("one"); });
    expect(result.current.activeDatabaseName).toBe("beta");
    expect(result.current.activeSchema).toBe("beta");
  });

  it("does not announce an unsaved default when persistence fails", async () => {
    const { result } = renderHook(useDatabase, { wrapper });
    await act(async () => { await result.current.connect("one"); });
    vi.mocked(invoke).mockRejectedValueOnce(new Error("disk write failed"));
    await act(async () => { await expect(result.current.setDefaultDatabase("one", "beta")).rejects.toThrow("disk write failed"); });
    expect(result.current.selectedDatabases).toEqual(["alpha", "beta", "gamma"]);
    expect(result.current.activeDatabaseName).toBe("alpha");
  });

  it("targets the menu's captured connection even after another connection is focused", async () => {
    const { result } = renderHook(useDatabase, { wrapper });
    await act(async () => { await result.current.connect("one"); });
    await act(async () => { await result.current.connect("two"); });
    await act(async () => { await result.current.setDefaultDatabase("one", "gamma"); });
    expect(result.current.activeConnectionId).toBe("two");
    expect(result.current.activeDatabaseName).toBe("alpha");
    expect(result.current.connectionDataMap.one.databaseName).toBe("gamma");
  });

  it("is idempotent for the current default and rejects unknown databases before saving", async () => {
    const { result } = renderHook(useDatabase, { wrapper });
    await act(async () => { await result.current.connect("one"); });
    vi.mocked(invoke).mockClear();
    await act(async () => { await result.current.setDefaultDatabase("one", "alpha"); });
    await act(async () => { await expect(result.current.setDefaultDatabase("one", "missing")).rejects.toThrow(); });
    expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "set_selected_databases")).toBe(false);
  });
});
