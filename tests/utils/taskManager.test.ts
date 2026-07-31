import { describe, expect, it } from "vitest";
import {
  formatBytes,
  formatCpuPercent,
  formatMemoryBar,
} from "../../src/utils/taskManager";

describe("taskManager utils", () => {
  it("formats byte counts across units", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(-100)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(2.5 * 1024 * 1024)).toBe("2.5 MB");
    expect(formatBytes(2 * 1024 * 1024 * 1024)).toBe("2.00 GB");
  });

  it("clamps and formats CPU percentages", () => {
    expect(formatCpuPercent(-5)).toBe("0.0%");
    expect(formatCpuPercent(33.3)).toBe("33.3%");
    expect(formatCpuPercent(150)).toBe("100.0%");
  });

  it("calculates a bounded memory percentage", () => {
    expect(formatMemoryBar(100, 0)).toBe(0);
    expect(formatMemoryBar(512, 1024)).toBe(50);
    expect(formatMemoryBar(2048, 1024)).toBe(100);
  });
});
