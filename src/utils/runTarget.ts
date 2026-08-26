/** What the Run button (and Ctrl/Cmd+Enter) executes for the editor state. */
export type RunTarget = "selection" | "all";

/** Editor state the run target is derived from, as reported by the editor. */
export interface RunContext {
  hasSelection: boolean;
  statementCount: number;
}

export function resolveRunTarget(options: RunContext): RunTarget {
  if (options.hasSelection) return "selection";
  return "all";
}
