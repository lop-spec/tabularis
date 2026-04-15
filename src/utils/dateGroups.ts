/**
 * Groups items by date category: Today, Yesterday, This Week, This Month, Older.
 * Returns an array of [groupKey, items[]] tuples in chronological group order.
 */
export function groupByDate<T>(
  items: T[],
  getDate: (item: T) => string,
): [string, T[]][] {
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const yesterday = new Date(today);
  yesterday.setDate(yesterday.getDate() - 1);
  const weekAgo = new Date(today);
  weekAgo.setDate(weekAgo.getDate() - 7);
  const monthAgo = new Date(today);
  monthAgo.setMonth(monthAgo.getMonth() - 1);

  const groups = new Map<string, T[]>();
  const order = [
    "dateGroupToday",
    "dateGroupYesterday",
    "dateGroupThisWeek",
    "dateGroupThisMonth",
    "dateGroupOlder",
  ];

  for (const key of order) {
    groups.set(key, []);
  }

  for (const item of items) {
    const date = new Date(getDate(item));
    const dateOnly = new Date(
      date.getFullYear(),
      date.getMonth(),
      date.getDate(),
    );

    let groupKey: string;
    if (dateOnly >= today) {
      groupKey = "dateGroupToday";
    } else if (dateOnly >= yesterday) {
      groupKey = "dateGroupYesterday";
    } else if (dateOnly >= weekAgo) {
      groupKey = "dateGroupThisWeek";
    } else if (dateOnly >= monthAgo) {
      groupKey = "dateGroupThisMonth";
    } else {
      groupKey = "dateGroupOlder";
    }

    groups.get(groupKey)!.push(item);
  }

  // Return only non-empty groups
  return order
    .filter((key) => groups.get(key)!.length > 0)
    .map((key) => [key, groups.get(key)!]);
}

/**
 * Formats a timestamp for display in the history sidebar.
 * Today: "HH:mm", otherwise: "MMM DD"
 */
export function formatHistoryTime(isoDate: string): string {
  const date = new Date(isoDate);
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const dateOnly = new Date(
    date.getFullYear(),
    date.getMonth(),
    date.getDate(),
  );

  if (dateOnly >= today) {
    return date.toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  return date.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}
