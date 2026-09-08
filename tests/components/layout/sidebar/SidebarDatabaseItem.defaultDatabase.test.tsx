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
