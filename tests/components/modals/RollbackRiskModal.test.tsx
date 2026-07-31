import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { RollbackRiskModal } from "../../../src/components/modals/RollbackRiskModal";

vi.mock("../../../src/components/ui/Modal", () => ({
  Modal: ({
    isOpen,
    children,
  }: {
    isOpen: boolean;
    children: React.ReactNode;
  }) => (isOpen ? <div data-testid="modal">{children}</div> : null),
}));

describe("RollbackRiskModal", () => {
  const onCancel = vi.fn();
  const onSkip = vi.fn();
  const onExecuteUnprotected = vi.fn();
  const review = {
    statements: [
      {
        index: 2,
        sql: "INSERT INTO users (id) SELECT id FROM staging",
        reason: "无法确定精确插入行",
        destructive: false,
      },
      {
        index: 4,
        sql: "TRUNCATE TABLE users",
        reason: "无法重建被删除的数据",
        destructive: true,
      },
    ],
  };

  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("shows all risks and allows unsupported statements to be skipped", () => {
    render(
      <RollbackRiskModal
        isOpen
        review={review}
        onCancel={onCancel}
        onSkip={onSkip}
        onExecuteUnprotected={onExecuteUnprotected}
      />,
    );

    expect(
      screen.getAllByText("editor.rollbackRiskStatement"),
    ).toHaveLength(2);
    expect(screen.getByText("无法确定精确插入行")).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("button", { name: "editor.rollbackRiskSkip" }),
    );
    expect(onSkip).toHaveBeenCalledTimes(1);
    expect(onExecuteUnprotected).not.toHaveBeenCalled();
  });

  it("requires a countdown before executing without rollback protection", () => {
    render(
      <RollbackRiskModal
        isOpen
        review={review}
        onCancel={onCancel}
        onSkip={onSkip}
        onExecuteUnprotected={onExecuteUnprotected}
        riskDelaySeconds={3}
      />,
    );

    expect(
      screen.getByRole("button", {
        name: /editor\.rollbackRiskExecute \(3\)/,
      }),
    ).toBeDisabled();
    act(() => vi.advanceTimersByTime(3000));
    const executeButton = screen.getByRole("button", {
      name: "editor.rollbackRiskExecute",
    });
    expect(executeButton).not.toBeDisabled();
    fireEvent.click(executeButton);
    expect(onExecuteUnprotected).toHaveBeenCalledTimes(1);
  });

  it("keeps cancel as a separate safe default", () => {
    render(
      <RollbackRiskModal
        isOpen
        review={review}
        onCancel={onCancel}
        onSkip={onSkip}
        onExecuteUnprotected={onExecuteUnprotected}
      />,
    );

    fireEvent.click(
      screen.getByRole("button", { name: "editor.rollbackRiskCancel" }),
    );
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onSkip).not.toHaveBeenCalled();
    expect(onExecuteUnprotected).not.toHaveBeenCalled();
  });
});
