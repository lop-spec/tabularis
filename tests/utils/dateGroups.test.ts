import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { groupByDate, formatHistoryTime } from "../../src/utils/dateGroups";

describe("dateGroups", () => {
  describe("groupByDate", () => {
    // Fix "now" to 2026-04-15 14:00:00 for deterministic tests
    const NOW = new Date(2026, 3, 15, 14, 0, 0);

    beforeEach(() => {
      vi.useFakeTimers();
      vi.setSystemTime(NOW);
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    const makeItem = (dateStr: string) => ({ date: dateStr, value: dateStr });
    const getDate = (item: { date: string }) => item.date;

    it("should return empty array for empty input", () => {
      const result = groupByDate([], getDate);
      expect(result).toEqual([]);
    });

    it("should group items from today", () => {
      const items = [
        makeItem("2026-04-15T10:00:00.000Z"),
        makeItem("2026-04-15T08:30:00.000Z"),
      ];
      const result = groupByDate(items, getDate);
      expect(result).toHaveLength(1);
      expect(result[0][0]).toBe("dateGroupToday");
      expect(result[0][1]).toHaveLength(2);
    });

    it("should group items from yesterday", () => {
      // Use midday to avoid timezone edge cases
      const items = [makeItem("2026-04-14T12:00:00.000Z")];
      const result = groupByDate(items, getDate);
      expect(result).toHaveLength(1);
      expect(result[0][0]).toBe("dateGroupYesterday");
      expect(result[0][1]).toHaveLength(1);
    });

    it("should group items from this week", () => {
      // 5 days ago (April 10)
      const items = [makeItem("2026-04-10T12:00:00.000Z")];
      const result = groupByDate(items, getDate);
      expect(result).toHaveLength(1);
      expect(result[0][0]).toBe("dateGroupThisWeek");
    });

    it("should group items from this month", () => {
      // 20 days ago (March 26)
      const items = [makeItem("2026-03-26T12:00:00.000Z")];
      const result = groupByDate(items, getDate);
      expect(result).toHaveLength(1);
      expect(result[0][0]).toBe("dateGroupThisMonth");
    });

    it("should group old items", () => {
      // 2 months ago
      const items = [makeItem("2026-02-10T12:00:00.000Z")];
      const result = groupByDate(items, getDate);
      expect(result).toHaveLength(1);
      expect(result[0][0]).toBe("dateGroupOlder");
    });

    it("should group items into multiple categories", () => {
      const items = [
        makeItem("2026-04-15T09:00:00.000Z"), // today
        makeItem("2026-04-14T18:00:00.000Z"), // yesterday
        makeItem("2026-04-12T10:00:00.000Z"), // this week
        makeItem("2026-03-20T10:00:00.000Z"), // this month
        makeItem("2026-01-05T10:00:00.000Z"), // older
      ];
      const result = groupByDate(items, getDate);

      expect(result).toHaveLength(5);
      expect(result[0][0]).toBe("dateGroupToday");
      expect(result[1][0]).toBe("dateGroupYesterday");
      expect(result[2][0]).toBe("dateGroupThisWeek");
      expect(result[3][0]).toBe("dateGroupThisMonth");
      expect(result[4][0]).toBe("dateGroupOlder");
    });

    it("should skip empty groups", () => {
      const items = [
        makeItem("2026-04-15T09:00:00.000Z"), // today
        makeItem("2026-01-05T10:00:00.000Z"), // older
      ];
      const result = groupByDate(items, getDate);
      expect(result).toHaveLength(2);
      expect(result[0][0]).toBe("dateGroupToday");
      expect(result[1][0]).toBe("dateGroupOlder");
    });

    it("should preserve item order within groups", () => {
      const items = [
        makeItem("2026-04-15T14:00:00.000Z"),
        makeItem("2026-04-15T10:00:00.000Z"),
        makeItem("2026-04-15T08:00:00.000Z"),
      ];
      const result = groupByDate(items, getDate);
      expect(result[0][1]).toEqual(items);
    });
  });

  describe("formatHistoryTime", () => {
    const NOW = new Date(2026, 3, 15, 14, 0, 0);

    beforeEach(() => {
      vi.useFakeTimers();
      vi.setSystemTime(NOW);
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it("should format today timestamps as HH:mm", () => {
      const result = formatHistoryTime("2026-04-15T10:30:00.000Z");
      // The exact format depends on locale but should contain hours and minutes
      expect(result).toMatch(/\d{1,2}:\d{2}/);
    });

    it("should format older timestamps as month + day", () => {
      const result = formatHistoryTime("2026-03-10T10:30:00.000Z");
      // Should contain "Mar" or locale equivalent and "10"
      expect(result).toContain("10");
    });

    it("should format yesterday timestamps as month + day", () => {
      const result = formatHistoryTime("2026-04-14T10:30:00.000Z");
      expect(result).toContain("14");
    });
  });
});
