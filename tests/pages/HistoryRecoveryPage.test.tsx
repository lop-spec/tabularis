import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { HistoryRecoveryPage } from "../../src/pages/HistoryRecoveryPage";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  loadAllHistory: vi.fn(),
  searchAllHistory: vi.fn(),
  pathname: "/history",
  connections: [
    {
      id: "mysql-backup",
      name: "MySQL backup",
      params: {
        driver: "mysql",
        host: "backup.example.test",
        port: 3306,
        username: "backup_user",
        password: "must-not-be-sent",
        database: "app_backup",
      },
    },
    {
      id: "postgres-backup",
      name: "PostgreSQL backup",
      params: {
        driver: "postgresql",
        host: "postgres.example.test",
        port: 5432,
        database: "app_backup",
      },
    },
  ],
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: mocks.invoke,
}));

// Icons render as nothing.
vi.mock("lucide-react", () => ({
  AlertTriangle: () => null,
  CheckCircle2: () => null,
  ChevronDown: () => null,
  ChevronRight: () => null,
  Clock3: () => null,
  Copy: () => null,
  Database: () => null,
  FileText: () => null,
  History: () => null,
  Loader2: () => null,
  RefreshCw: () => null,
  RotateCcw: () => null,
  Search: () => null,
  ShieldCheck: () => null,
  X: () => null,
  XCircle: () => null,
}));

vi.mock("react-router-dom", () => ({
  useLocation: () => ({ pathname: mocks.pathname }),
}));

vi.mock("../../src/hooks/useDatabase", () => ({
  useDatabase: () => ({
    activeConnectionId: "mysql-target",
    connections: mocks.connections,
  }),
}));

vi.mock("../../src/hooks/useQueryHistory", () => ({
  useQueryHistory: () => ({
    loadAllHistory: mocks.loadAllHistory,
    searchAllHistory: mocks.searchAllHistory,
  }),
}));

const historyEntry = {
  id: "h-1",
  sql: "SELECT 1 FROM information_schema.tables",
  executedAt: "2026-08-07T10:00:00.000Z",
  executionTimeMs: 12,
  status: "success" as const,
  rowsAffected: 1,
  error: null,
  database: "csr",
  connectionId: "mysql-target",
  connectionName: "MySQL target",
};

const recoveryRun = {
  runId: "run-1",
  shortId: "RUN-1",
  startedAt: "2026-07-31T01:00:00.000Z",
  finishedAt: "2026-07-31T01:00:01.000Z",
  status: "complete",
  connectionId: "mysql-target",
  connectionName: "MySQL target",
  database: "app",
  statementCount: 2,
  statements: [
    {
      id: "statement-1",
      index: 0,
      executedAt: "2026-07-31T01:00:00.000Z",
      sql: "UPDATE users SET name = 'new' WHERE id = 1",
      category: "dml",
      operation: "update",
      schema: "app",
      table: "users",
      affectedColumns: ["name"],
      condition: "id = 1",
      rowCount: 1,
      exact: true,
    },
    {
      id: "statement-2",
      index: 1,
      executedAt: "2026-07-31T01:00:00.500Z",
      sql: "REPLACE INTO users (id) VALUES (9)",
      category: "unprotected",
      operation: "replace",
      schema: "app",
      table: "users",
      affectedColumns: [],
      condition: null,
      rowCount: 0,
      exact: false,
    },
  ],
};

const offlineResponse = {
  outputPath: "C:/offline-rollback.sql",
  sql: "-- offline rollback",
  generatedSteps: 1,
  unchangedRows: 0,
  conflicts: [],
  exact: true,
  targetInstance: "MySQL target · uuid:abc",
  backupInstance: "recorded row images (offline)",
};

describe("HistoryRecoveryPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.pathname = "/history";
    mocks.loadAllHistory.mockResolvedValue([historyEntry]);
    mocks.searchAllHistory.mockResolvedValue([]);
    mocks.invoke.mockImplementation((command: string) => {
      if (command === "list_recovery_runs") return Promise.resolve([recoveryRun]);
      if (command === "generate_offline_recovery_sql")
        return Promise.resolve(offlineResponse);
      if (command === "generate_recovery_sql") return Promise.resolve(offlineResponse);
      if (command === "test_saved_connection") return Promise.resolve("ok");
      return Promise.resolve(undefined);
    });
  });

  it("shows the execution history list with entry details on click", async () => {
    render(<HistoryRecoveryPage />);

    const row = await screen.findByText(/SELECT 1 FROM information_schema/, {
      selector: "pre",
    });
    fireEvent.click(row.closest("button")!);

    expect(screen.getByText("Copy SQL")).toBeInTheDocument();
    expect(screen.getAllByText(/MySQL target/).length).toBeGreaterThan(0);
  });

  it("opens the recovery tab when routed to /recovery", async () => {
    mocks.pathname = "/recovery";
    render(<HistoryRecoveryPage />);

    await waitFor(() => {
      expect(mocks.invoke).toHaveBeenCalledWith(
        "list_recovery_runs",
        expect.objectContaining({ connectionId: "mysql-target" }),
      );
    });
    expect(screen.getByRole("tab", { name: "Recoverable changes" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("generates offline rollback SQL for an exact selection without a backup connection", async () => {
    mocks.pathname = "/recovery";
    render(<HistoryRecoveryPage />);

    fireEvent.click(await screen.findByText(/UPDATE users SET name/));
    const generateButton = screen.getByRole("button", {
      name: "Generate rollback SQL",
    });
    expect(generateButton).toBeEnabled();
    fireEvent.click(generateButton);

    await waitFor(() => {
      expect(mocks.invoke).toHaveBeenCalledWith("generate_offline_recovery_sql", {
        connectionId: "mysql-target",
        selection: { runIds: ["run-1"], statementIds: ["statement-1"] },
      });
    });
    expect(await screen.findByText("-- offline rollback")).toBeInTheDocument();
    expect(screen.getByText(/offline-rollback\.sql/)).toBeInTheDocument();
  });

  it("blocks the offline action when the selection contains inexact statements", async () => {
    mocks.pathname = "/recovery";
    render(<HistoryRecoveryPage />);

    fireEvent.click(await screen.findByText(/REPLACE INTO users/));
    const generateButton = screen.getByRole("button", {
      name: "Generate rollback SQL",
    });

    expect(generateButton).toBeDisabled();
    expect(
      screen.getAllByText(/use the backup comparison/i).length,
    ).toBeGreaterThan(0);
  });

  it("runs the backup comparison from the advanced section without leaking credentials", async () => {
    mocks.pathname = "/recovery";
    render(<HistoryRecoveryPage />);

    fireEvent.click(await screen.findByText(/REPLACE INTO users/));
    fireEvent.click(screen.getByText(/Backup-instance comparison/));

    const select = screen.getByLabelText("Saved connection");
    expect(screen.getByRole("option", { name: /MySQL backup/ })).toBeInTheDocument();
    expect(
      screen.queryByRole("option", { name: /PostgreSQL backup/ }),
    ).not.toBeInTheDocument();
    fireEvent.change(select, { target: { value: "mysql-backup" } });
    fireEvent.click(
      screen.getByRole("button", { name: "Compare read-only and generate" }),
    );

    await waitFor(() => {
      expect(mocks.invoke).toHaveBeenCalledWith("generate_recovery_sql", {
        connectionId: "mysql-target",
        selection: { runIds: ["run-1"], statementIds: ["statement-2"] },
        backupConnectionId: "mysql-backup",
      });
    });
    expect(JSON.stringify(mocks.invoke.mock.calls)).not.toContain("must-not-be-sent");
  });

  it("searches recoverable changes through the shared search box", async () => {
    mocks.pathname = "/recovery";
    render(<HistoryRecoveryPage />);
    await screen.findByText(/UPDATE users SET name/);

    fireEvent.change(
      screen.getByLabelText(/Search recoverable changes/),
      { target: { value: "UPDATE users" } },
    );

    await waitFor(() => {
      expect(mocks.invoke).toHaveBeenCalledWith(
        "list_recovery_runs",
        expect.objectContaining({ query: "UPDATE users" }),
      );
    });
  });
});
