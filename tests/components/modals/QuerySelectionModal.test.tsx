import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { QuerySelectionModal } from "../../../src/components/modals/QuerySelectionModal";

// Mock the Modal component to just render children
vi.mock("../../../src/components/ui/Modal", () => ({
  Modal: ({
    isOpen,
    children,
  }: {
    isOpen: boolean;
    children: React.ReactNode;
  }) => (isOpen ? <div data-testid="modal">{children}</div> : null),
}));

describe("QuerySelectionModal", () => {
  const queries = ["SELECT * FROM users", "SELECT * FROM posts", "SELECT 1"];
  const mockOnSelect = vi.fn();
  const mockOnRunAll = vi.fn();
  const mockOnRunSelected = vi.fn();
  const mockOnClose = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  const renderModal = (isOpen = true) =>
    render(
      <QuerySelectionModal
        isOpen={isOpen}
        queries={queries}
        onSelect={mockOnSelect}
        onRunAll={mockOnRunAll}
        onRunSelected={mockOnRunSelected}
        onClose={mockOnClose}
      />,
    );

  it("does not render when isOpen is false", () => {
    renderModal(false);
    expect(screen.queryByTestId("modal")).not.toBeInTheDocument();
  });

  it("renders all queries when open", () => {
    renderModal();
    expect(screen.getByText("SELECT * FROM users")).toBeInTheDocument();
    expect(screen.getByText("SELECT * FROM posts")).toBeInTheDocument();
    expect(screen.getByText("SELECT 1")).toBeInTheDocument();
  });

  it("renders the title", () => {
    renderModal();
    expect(
      screen.getByText("editor.querySelection.title"),
    ).toBeInTheDocument();
  });

  it("renders query count in footer", () => {
    renderModal();
    // The i18n mock returns the key with interpolation
    expect(
      screen.getByText(/editor\.querySelection\.queriesFound/),
    ).toBeInTheDocument();
  });

  it("renders Run All button", () => {
    renderModal();
    expect(
      screen.getByText("editor.querySelection.runAll"),
    ).toBeInTheDocument();
  });

  it("renders Run Selected button", () => {
    renderModal();
    expect(
      screen.getByText(/editor\.querySelection\.runSelected/),
    ).toBeInTheDocument();
  });

  it("calls onRunAll with all queries when Run All is clicked", () => {
    renderModal();
    fireEvent.click(screen.getByText("editor.querySelection.runAll"));
    expect(mockOnRunAll).toHaveBeenCalledWith(queries);
  });

  it("calls onClose when close button is clicked", () => {
    renderModal();
    // The header contains h3 "title" and a div with selectAll button + close button.
    // Both are direct children in the header flex container.
    const allButtons = screen.getAllByRole("button");
    // The close button is inside the header, right after the selectAll button.
    // It's the second button in the DOM (first is selectAll).
    // Find the button that has the selectAll text, the next sibling is close.
    const selectAllBtn = screen.getByText("editor.querySelection.selectAll");
    const closeBtn = selectAllBtn.closest("div")?.querySelector("button:last-child");
    expect(closeBtn).not.toBeNull();
    fireEvent.click(closeBtn!);
    expect(mockOnClose).toHaveBeenCalled();
  });

  it("calls onSelect when clicking on a query text", () => {
    renderModal();
    fireEvent.click(screen.getByText("SELECT * FROM users"));
    expect(mockOnSelect).toHaveBeenCalledWith("SELECT * FROM users");
  });

  it("Run Selected is disabled when no queries are selected", () => {
    renderModal();
    const runSelectedBtn = screen
      .getByText(/editor\.querySelection\.runSelected/)
      .closest("button");
    expect(runSelectedBtn).toBeDisabled();
  });

  it("calls onRunAll on Ctrl+Enter keydown", () => {
    renderModal();
    fireEvent.keyDown(window, {
      key: "Enter",
      ctrlKey: true,
    });
    expect(mockOnRunAll).toHaveBeenCalledWith(queries);
  });

  it("calls onSelect on Enter keydown (single query)", () => {
    renderModal();
    fireEvent.keyDown(window, { key: "Enter" });
    // Focus starts on index 0
    expect(mockOnSelect).toHaveBeenCalledWith("SELECT * FROM users");
  });

  it("navigates focus with arrow keys", () => {
    renderModal();
    // Move focus down
    fireEvent.keyDown(window, { key: "ArrowDown" });
    // Now pressing Enter should select the second query
    fireEvent.keyDown(window, { key: "Enter" });
    expect(mockOnSelect).toHaveBeenCalledWith("SELECT * FROM posts");
  });

  it("selects query by number key", () => {
    renderModal();
    fireEvent.keyDown(window, { key: "2" });
    expect(mockOnSelect).toHaveBeenCalledWith("SELECT * FROM posts");
  });

  it("toggles checkbox selection with Space key", () => {
    renderModal();
    // Press space to toggle selection on first query
    fireEvent.keyDown(window, { key: " " });
    // Now Run Selected should be enabled — click it
    const runSelectedBtn = screen
      .getByText(/editor\.querySelection\.runSelected/)
      .closest("button");
    expect(runSelectedBtn).not.toBeDisabled();
    fireEvent.click(runSelectedBtn!);
    expect(mockOnRunSelected).toHaveBeenCalledWith(["SELECT * FROM users"]);
  });

  it("shows Select All / Deselect All toggle", () => {
    renderModal();
    expect(
      screen.getByText("editor.querySelection.selectAll"),
    ).toBeInTheDocument();
  });

  it("toggles all selections when Select All is clicked", () => {
    renderModal();
    // Click Select All
    fireEvent.click(screen.getByText("editor.querySelection.selectAll"));
    // Now it should show Deselect All
    expect(
      screen.getByText("editor.querySelection.deselectAll"),
    ).toBeInTheDocument();
    // Click Run Selected — should include all queries
    const runSelectedBtn = screen
      .getByText(/editor\.querySelection\.runSelected/)
      .closest("button");
    fireEvent.click(runSelectedBtn!);
    expect(mockOnRunSelected).toHaveBeenCalledWith(queries);
  });
});
