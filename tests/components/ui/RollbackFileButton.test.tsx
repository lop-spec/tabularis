import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { RollbackFileButton } from "../../../src/components/ui/RollbackFileButton";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => {
      if (key === "editor.rollbackButton") return "Rollback";
      if (key === "editor.viewRollbackSql") return "View rollback.sql";
      return key;
    },
  }),
}));

describe("RollbackFileButton", () => {
  it("is absent when the current tab has no generated rollback file", () => {
    render(<RollbackFileButton onOpen={vi.fn()} />);

    expect(
      screen.queryByRole("button", { name: "View rollback.sql" }),
    ).not.toBeInTheDocument();
  });

  it("opens the exact generated rollback file and prevents duplicate clicks", async () => {
    let resolveOpen: (() => void) | undefined;
    const onOpen = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolveOpen = resolve;
        }),
    );
    render(
      <RollbackFileButton
        rollbackFile="C:/Tabularis/run.rollback.sql"
        onOpen={onOpen}
      />,
    );

    const button = screen.getByRole("button", {
      name: "View rollback.sql",
    });
    expect(screen.getByText("Rollback")).toBeInTheDocument();

    fireEvent.click(button);
    fireEvent.click(button);

    expect(onOpen).toHaveBeenCalledTimes(1);
    expect(onOpen).toHaveBeenCalledWith("C:/Tabularis/run.rollback.sql");
    expect(button).toBeDisabled();

    resolveOpen?.();
    await waitFor(() => expect(button).not.toBeDisabled());
  });
});
