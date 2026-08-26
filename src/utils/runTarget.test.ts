import { describe, it, expect } from "vitest";
import { resolveRunTarget } from "./runTarget";

describe("resolveRunTarget", () => {
  it("runs the selection whenever one is active", () => {
    expect(
      resolveRunTarget({
        hasSelection: true,
        statementCount: 12,
      }),
    ).toBe("selection");
  });

  it("defaults an empty editor to Run All", () => {
    expect(
      resolveRunTarget({
        hasSelection: false,
        statementCount: 0,
      }),
    ).toBe("all");
  });

  it("defaults a single statement to Run All", () => {
    expect(
      resolveRunTarget({
        hasSelection: false,
        statementCount: 1,
      }),
    ).toBe("all");
  });

  it("defaults a multi-statement script to Run All", () => {
    expect(
      resolveRunTarget({
        hasSelection: false,
        statementCount: 21,
      }),
    ).toBe("all");
  });
});
