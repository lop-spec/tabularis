import { describe, expect, it } from "vitest";
import { getDefaultEditorHeight } from "../../src/utils/editorLayout";

describe("editorLayout", () => {
  it("leaves one third of the full editor pane for results by default", () => {
    expect(getDefaultEditorHeight()).toBe("calc(66.666667% - 86px)");
  });
});
