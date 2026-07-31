import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DataGrid } from "../../../src/components/ui/DataGrid";

vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: ({ count }: { count: number }) => ({
    getVirtualItems: () =>
      Array.from({ length: count }, (_, index) => ({
        index,
        key: index,
        start: index * 35,
        end: (index + 1) * 35,
        size: 35,
        lane: 0,
      })),
    getTotalSize: () => count * 35,
    scrollToIndex: vi.fn(),
  }),
}));

vi.mock("../../../src/hooks/useDatabase", () => ({
  useDatabase: () => ({ activeSchema: null, connections: [] }),
}));

vi.mock("../../../src/hooks/useAlert", () => ({
  useAlert: () => ({ showAlert: vi.fn() }),
}));

vi.mock("../../../src/hooks/useSettings", () => ({
  useSettings: () => ({
    settings: {
      resultColorByType: false,
      stickyColumnHeaders: true,
    },
  }),
}));

vi.mock("../../../src/hooks/useRightSidebar", () => ({
  useRightSidebar: () => ({
    isOpen: false,
    activePanel: null,
    rowEditorData: null,
    isPinned: false,
    openRowEditor: vi.fn(),
    updateRowEditorData: vi.fn(),
    close: vi.fn(),
    toggle: vi.fn(),
    setActivePanel: vi.fn(),
    togglePin: vi.fn(),
    onChangeRef: { current: null },
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => undefined),
}));

const writeText = vi.fn().mockResolvedValue(undefined);

beforeEach(() => {
  vi.clearAllMocks();
  Object.defineProperty(globalThis, "ResizeObserver", {
    configurable: true,
    value: class ResizeObserver {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  });
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText },
  });
});

function renderGrid() {
  return render(
    <div style={{ height: 400 }}>
      <DataGrid
        columns={["id", "name", "role"]}
        data={[
          [1, "Alice", "admin"],
          [2, "Bob", "viewer"],
        ]}
        copyFormat="csv"
        csvDelimiter=","
        csvIncludeHeaders
      />
    </div>,
  );
}

describe("DataGrid layout", () => {
  it("keeps hidden header tooltips out of scrollable overflow", () => {
    const { container } = render(
      <DataGrid
        columns={["id", "name"]}
        data={[[1, "Alice"]]}
        columnMetadata={[
          {
            name: "id",
            data_type: "integer",
            is_pk: true,
            is_nullable: false,
            is_auto_increment: false,
          },
          {
            name: "name",
            data_type: "character varying(255)",
            is_pk: false,
            is_nullable: false,
            is_auto_increment: false,
          },
        ]}
        selectedRows={new Set()}
        onSelectionChange={vi.fn()}
        readonly
      />,
    );

    const table = container.querySelector("table");
    const tooltips = container.querySelectorAll('[role="tooltip"]');

    expect(table).toHaveClass("w-full");
    expect(tooltips).toHaveLength(2);
    expect(tooltips[0]).toHaveClass("hidden", "left-0");
    expect(tooltips[1]).toHaveClass("hidden", "right-0");
    expect(tooltips[1]).not.toHaveClass("left-0");
  });
});

describe("DataGrid selection and clipboard behavior", () => {
  it("selects one full row normally and adds rows only with Ctrl", () => {
    renderGrid();
    const rows = screen.getByRole("table").querySelectorAll("tbody tr");
    const firstRowNumber = rows[0].querySelector("td:first-child");
    const secondRowNumber = rows[1].querySelector("td:first-child");

    fireEvent.click(firstRowNumber!);
    expect(rows[0]).toHaveAttribute("aria-selected", "true");
    expect(rows[1]).toHaveAttribute("aria-selected", "false");

    fireEvent.click(secondRowNumber!);
    expect(rows[0]).toHaveAttribute("aria-selected", "false");
    expect(rows[1]).toHaveAttribute("aria-selected", "true");

    fireEvent.click(firstRowNumber!, { ctrlKey: true });
    expect(rows[0]).toHaveAttribute("aria-selected", "true");
    expect(rows[1]).toHaveAttribute("aria-selected", "true");

    fireEvent.click(secondRowNumber!, { ctrlKey: true });
    expect(rows[0]).toHaveAttribute("aria-selected", "true");
    expect(rows[1]).toHaveAttribute("aria-selected", "false");
  });

  it("selects and copies all rows when the # header is clicked", async () => {
    renderGrid();
    const table = screen.getByRole("table");
    const rows = table.querySelectorAll("tbody tr");
    const selectAllHeader = screen.getByText("#").closest("th");

    fireEvent.click(selectAllHeader!);

    expect(rows[0]).toHaveAttribute("aria-selected", "true");
    expect(rows[1]).toHaveAttribute("aria-selected", "true");
    await waitFor(() =>
      expect(writeText).toHaveBeenCalledWith(
        "id,name,role\n1,Alice,admin\n2,Bob,viewer",
      ),
    );
  });

  it("copies a dragged rectangular range with CSV headers", async () => {
    renderGrid();
    const firstCell = screen.getByText("Alice").closest("td");
    const lastCell = screen.getByText("viewer").closest("td");
    const grid = screen.getByRole("table").parentElement;

    fireEvent.mouseDown(firstCell!, { button: 0, buttons: 1 });
    fireEvent.mouseEnter(lastCell!, { buttons: 1 });
    fireEvent.mouseUp(document);

    expect(
      screen.getByRole("table").querySelectorAll('td[aria-selected="true"]'),
    ).toHaveLength(4);

    fireEvent.keyDown(grid!, { key: "c", ctrlKey: true });

    await waitFor(() =>
      expect(writeText).toHaveBeenCalledWith(
        "name,role\nAlice,admin\nBob,viewer",
      ),
    );
  });
});
