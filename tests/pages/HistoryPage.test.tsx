import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { HistoryPage } from "../../src/pages/HistoryPage";

const mocks = vi.hoisted(() => ({
  loadAllHistory: vi.fn(),
  searchAllHistory: vi.fn(),
  searchHistory: vi.fn(),
}));

vi.mock("lucide-react", () => ({
  AlertCircle: () => null,
  History: () => null,
  Loader2: () => null,
  Search: () => null,
  X: () => null,
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) =>
      options && "count" in options ? `${key}:${options.count}` : key,
  }),
}));

vi.mock("../../src/hooks/useQueryHistory", () => ({
  useQueryHistory: () => mocks,
}));

const entry = (overrides: Record<string, unknown> = {}) => ({
  id: "1",
  sql: "SELECT 1",
  executedAt: "2026-08-07T10:00:00.000Z",
  executionTimeMs: 12,
  status: "success" as const,
  rowsAffected: 1,
  error: null,
  database: "csr",
  ...overrides,
});

describe("HistoryPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.loadAllHistory.mockResolvedValue([]);
    mocks.searchAllHistory.mockResolvedValue([]);
  });

  // The page used to render "no connection selected" whenever nothing was
  // active, hiding every statement the user had ever run.
  it("loads history without an active connection", async () => {
    mocks.loadAllHistory.mockResolvedValue([
      entry({ id: "a", sql: "UPDATE orders SET x = 1" }),
    ]);

    render(<HistoryPage />);

    await waitFor(() => {
      expect(screen.getByText("UPDATE orders SET x = 1")).toBeTruthy();
    });
    expect(mocks.loadAllHistory).toHaveBeenCalled();
  });

  it("names the connection each statement came from", async () => {
    mocks.loadAllHistory.mockResolvedValue([
      entry({ id: "a", sql: "SELECT 1", connectionName: "mysql-prod" }),
      entry({ id: "b", sql: "SELECT 2", connectionName: "mysql-test" }),
    ]);

    render(<HistoryPage />);

    await waitFor(() => {
      expect(screen.getByText("· mysql-prod")).toBeTruthy();
    });
    expect(screen.getByText("· mysql-test")).toBeTruthy();
  });

  it("searches across all connections, not just the active one", async () => {
    mocks.loadAllHistory.mockResolvedValue([]);
    mocks.searchAllHistory.mockResolvedValue([
      entry({ id: "c", sql: "DELETE FROM audit" }),
    ]);

    render(<HistoryPage />);
    const input = await screen.findByPlaceholderText("history.searchPlaceholder");
    fireEvent.change(input, { target: { value: "DELETE" } });

    await waitFor(() => {
      expect(mocks.searchAllHistory).toHaveBeenCalledWith("DELETE", 500);
    });
    // The per-connection search must not be what this page uses.
    expect(mocks.searchHistory).not.toHaveBeenCalled();
  });
});
