import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ErrorDisplay } from "../../../src/components/ui/ErrorDisplay";

const translate = ((key: string) => key) as never;

describe("ErrorDisplay clipboard behavior", () => {
  const writeText = vi.fn().mockResolvedValue(undefined);

  beforeEach(() => {
    writeText.mockClear();
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
  });

  it("allows text selection and copies the complete error including hidden details", async () => {
    const error = "Unsupported path\n\nDriver detail";
    const { container } = render(<ErrorDisplay error={error} t={translate} />);

    expect(container.firstElementChild).toHaveClass("select-text");
    expect(screen.queryByText("Driver detail")).not.toBeInTheDocument();

    fireEvent.click(
      screen.getByRole("button", { name: "sidebar.copyError" }),
    );

    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith(
        "Error: Unsupported path\n\nDriver detail",
      );
    });
  });
});
