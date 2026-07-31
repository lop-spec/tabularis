import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { GenerateSQLModal } from "../../../src/components/modals/GenerateSQLModal";

const databaseState = vi.hoisted(() => ({
  activeConnectionId: "connection-1",
  activeDriver: "mysql",
  activeSchema: "active_db",
  activeCapabilities: {
    identifier_quote: "`",
    auto_increment_keyword: "AUTO_INCREMENT",
    serial_type: "",
    inline_pk: false,
  },
}));
const alertMocks = vi.hoisted(() => ({ showAlert: vi.fn() }));

vi.mock("../../../src/hooks/useDatabase", () => ({
  useDatabase: () => databaseState,
}));

vi.mock("../../../src/hooks/useAlert", () => ({
  useAlert: () => alertMocks,
}));

vi.mock("react-i18next", () => {
  const t = (key: string) => key;
  return { useTranslation: () => ({ t }) };
});

vi.mock("react-router-dom", async (importOriginal) => ({
  ...(await importOriginal<typeof import("react-router-dom")>()),
  useNavigate: () => vi.fn(),
}));

vi.mock("../../../src/components/ui/Modal", () => ({
  Modal: ({ isOpen, children }: { isOpen: boolean; children: ReactNode }) =>
    isOpen ? <div>{children}</div> : null,
}));

vi.mock("../../../src/components/ui/SqlPreview", () => ({
  SqlPreview: ({ sql }: { sql: string }) => <pre data-testid="sql-preview">{sql}</pre>,
}));

vi.mock("lucide-react", () => ({
  X: () => null,
  Loader2: () => null,
  Copy: () => null,
  Check: () => null,
  FileCode: () => null,
  List: () => null,
  Table2: () => null,
  PenLine: () => null,
  Trash2: () => null,
  Play: () => null,
}));

describe("GenerateSQLModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "get_columns") {
        return [
          {
            name: "id",
            data_type: "bigint",
            is_pk: true,
            is_nullable: false,
            is_auto_increment: true,
            default_value: null,
          },
        ];
      }
      if (command === "get_foreign_keys" || command === "get_indexes") return [];
      if (command === "execute_query") {
        return {
          columns: ["Table", "Create Table"],
          rows: [[
            "orders",
            "CREATE TABLE `orders` (`id` bigint NOT NULL AUTO_INCREMENT, PRIMARY KEY (`id`)) ENGINE=InnoDB AUTO_INCREMENT=42",
          ]],
          affected_rows: 0,
        };
      }
      throw new Error(`Unexpected command: ${command}`);
    });
  });

  it("loads MySQL CREATE TABLE SQL from SHOW CREATE TABLE for the clicked schema", async () => {
    render(
      <GenerateSQLModal
        isOpen={true}
        tableName="orders"
        schema="archive_db"
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId("sql-preview")).toHaveTextContent(
        "ENGINE=InnoDB AUTO_INCREMENT=42;",
      );
    });

    expect(invoke).toHaveBeenCalledWith("execute_query", {
      connectionId: "connection-1",
      query: "SHOW CREATE TABLE `archive_db`.`orders`;",
      schema: "archive_db",
    });
    expect(invoke).not.toHaveBeenCalledWith(
      "get_foreign_keys",
      expect.anything(),
    );
    expect(invoke).not.toHaveBeenCalledWith("get_indexes", expect.anything());
  });
});
