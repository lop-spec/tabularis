import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { RecoveryPage } from "../../src/pages/RecoveryPage";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
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
      id: "maria-backup",
      name: "MariaDB archive",
      params: {
        driver: "mariadb",
        host: "archive.example.test",
        port: 3307,
        database: "archive",
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

vi.mock("../../src/hooks/useDatabase", () => ({
  useDatabase: () => ({
    activeConnectionId: "mysql-target",
    connections: mocks.connections,
  }),
}));

vi.mock("lucide-react", () => ({
  AlertTriangle: () => null,
  CheckCircle2: () => null,
  Clock3: () => null,
  Copy: () => null,
  Database: () => null,
  FileText: () => null,
  Loader2: () => null,
  RefreshCw: () => null,
  ShieldCheck: () => null,
}));

const recoveryRun = {
  runId: "run-1",
  shortId: "RUN-1",
  startedAt: "2026-07-31T01:00:00.000Z",
  finishedAt: "2026-07-31T01:00:01.000Z",
  status: "completed",
  connectionId: "mysql-target",
  connectionName: "MySQL target",
  database: "app",
  statementCount: 1,
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
  ],
};

describe("RecoveryPage saved backup connection", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.invoke.mockImplementation((command: string) => {
      if (command === "list_recovery_runs") return Promise.resolve([recoveryRun]);
      if (command === "test_saved_connection") return Promise.resolve("ok");
      if (command === "generate_recovery_sql") {
        return Promise.resolve({
          outputPath: "C:/recovery.sql",
          sql: "-- recovery",
          generatedSteps: 1,
          unchangedRows: 0,
          conflicts: [],
          exact: true,
          targetInstance: "MySQL target",
          backupInstance: "MySQL backup",
        });
      }
      return Promise.resolve(undefined);
    });
  });

  it("selects a saved MySQL or MariaDB connection without manual credentials", async () => {
    render(<RecoveryPage />);

    const select = await screen.findByLabelText("Saved connection");
    expect(screen.getByRole("option", { name: /MySQL backup/ })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: /MariaDB archive/ })).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: /PostgreSQL backup/ })).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Host")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Password")).not.toBeInTheDocument();

    fireEvent.change(select, { target: { value: "mysql-backup" } });
    fireEvent.click(screen.getByRole("button", { name: "Test connection" }));

    await waitFor(() => {
      expect(mocks.invoke).toHaveBeenCalledWith("test_saved_connection", {
        connectionId: "mysql-backup",
      });
    });
    expect(JSON.stringify(mocks.invoke.mock.calls)).not.toContain("must-not-be-sent");
  });

  it("generates recovery SQL using only the selected backup connection id", async () => {
    render(<RecoveryPage />);

    fireEvent.change(await screen.findByLabelText("Saved connection"), {
      target: { value: "maria-backup" },
    });
    fireEvent.click(screen.getByText("Run All"));
    fireEvent.click(screen.getByRole("button", { name: "Compare read-only and generate SQL" }));

    await waitFor(() => {
      expect(mocks.invoke).toHaveBeenCalledWith("generate_recovery_sql", {
        connectionId: "mysql-target",
        selection: {
          runIds: ["run-1"],
          statementIds: ["statement-1"],
        },
        backupConnectionId: "maria-backup",
      });
    });
    expect(JSON.stringify(mocks.invoke.mock.calls)).not.toContain("must-not-be-sent");
  });
});
