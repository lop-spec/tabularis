import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ComponentProps } from "react";
import { SidebarDatabaseItem } from "../../../../src/components/layout/sidebar/SidebarDatabaseItem";

vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));

function props(): ComponentProps<typeof SidebarDatabaseItem> {
  return {
    databaseName: "beta", isDefault: true, databaseData: { tables: [], views: [], routines: [], triggers: [], isLoading: false, isLoaded: true },
    activeTable: null, activeSchema: "alpha", connectionId: "one", driver: "mysql", schemaVersion: 0,
    onLoadDatabase: vi.fn(), onRefreshDatabase: vi.fn(), onTableClick: vi.fn(), onTableDoubleClick: vi.fn(),
    onViewClick: vi.fn(), onViewDoubleClick: vi.fn(), onRoutineDoubleClick: vi.fn(), onTriggerDoubleClick: vi.fn(),
    onContextMenu: vi.fn(), onAddColumn: vi.fn(), onEditColumn: vi.fn(), onAddIndex: vi.fn(), onDropIndex: vi.fn(),
    onAddForeignKey: vi.fn(), onDropForeignKey: vi.fn(), onCreateTable: vi.fn(), onCreateView: vi.fn(), onCreateTrigger: vi.fn(),
  };
}

describe("database default indicator", () => {
  it("keeps the full database name in its own row, separate from counts and actions", () => {
    const input = props();
    const databaseName = "database_with_a_long_unbroken_identifier";
    render(<SidebarDatabaseItem {...input} databaseName={databaseName} />);
    const name = screen.getByText(databaseName);
    const counts = screen.getByText("0T / 0V / 0R");
    expect(name).toHaveAttribute("title", databaseName);
    expect(name).toHaveClass("flex-1", "min-w-0", "[overflow-wrap:anywhere]");
    expect(name).not.toHaveClass("truncate");
    expect(name.parentElement).not.toContainElement(counts);
    expect(name.parentElement).not.toContainElement(screen.getByTitle("sidebar.refreshTables"));
    expect(name.parentElement?.parentElement).toHaveClass("flex-col");
  });

  it("retains all actions without toggling the database when a button is clicked", () => {
    const input = { ...props(), onImport: vi.fn(), onDump: vi.fn(), onViewDiagram: vi.fn() };
    render(<SidebarDatabaseItem {...input} />);
    for (const [title, handler] of [
      ["dump.importDatabase", input.onImport],
      ["dump.dumpDatabase", input.onDump],
      ["sidebar.viewERDiagram", input.onViewDiagram],
      ["sidebar.refreshTables", input.onRefreshDatabase],
    ] as const) {
      fireEvent.click(screen.getByTitle(title));
      expect(handler).toHaveBeenCalledWith("beta");
    }
    expect(input.onLoadDatabase).not.toHaveBeenCalled();
  });

  it("marks the persisted default independently of the active database", () => {
    const input = props();
    const { rerender } = render(<SidebarDatabaseItem {...input} />);
    expect(screen.getByLabelText("sidebar.defaultDatabase")).toBeInTheDocument();
    rerender(<SidebarDatabaseItem {...input} isDefault={false} />);
    expect(screen.queryByLabelText("sidebar.defaultDatabase")).not.toBeInTheDocument();
  });

  it("opens the database context menu with the exact database name", () => {
    const input = props();
    render(<SidebarDatabaseItem {...input} />);
    fireEvent.contextMenu(screen.getByText("beta"));
    expect(input.onContextMenu).toHaveBeenCalledWith(expect.anything(), "database", "beta", "beta");
  });
});
