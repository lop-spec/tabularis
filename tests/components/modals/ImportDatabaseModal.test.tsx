import { render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ImportDatabaseModal } from "../../../src/components/modals/ImportDatabaseModal";

const { invokeMock, listenMock, showAlertMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
  showAlertMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock("../../../src/hooks/useDatabase", () => ({
  useDatabase: () => ({ activeSchema: "default_db" }),
}));
vi.mock("../../../src/hooks/useAlert", () => ({
  useAlert: () => ({ showAlert: showAlertMock }),
}));
vi.mock("../../../src/components/ui/Modal", () => ({
  Modal: ({ isOpen, children }: { isOpen: boolean; children: React.ReactNode }) =>
    isOpen ? <div>{children}</div> : null,
}));

describe("ImportDatabaseModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    invokeMock.mockResolvedValue(undefined);
    listenMock.mockResolvedValue(() => undefined);
  });

  it("imports into the database selected by the caller, not the global default", async () => {
    const filePath = "C:/tmp/release.sql";
    const onClose = vi.fn();
    render(
      <ImportDatabaseModal
        isOpen
        onClose={onClose}
        connectionId="conn-1"
        databaseName="right_click_db"
        targetDatabase="right_click_db"
        filePath={filePath}
      />,
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("import_database", {
        connectionId: "conn-1",
        filePath,
        schema: "right_click_db",
        database: "right_click_db",
      });
    });
  });
});
