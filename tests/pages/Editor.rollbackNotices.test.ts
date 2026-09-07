import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import ts from "typescript";

const source = readFileSync("src/pages/Editor.tsx", "utf8");
const tree = ts.createSourceFile("Editor.tsx", source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const alerts: string[] = [];
function visit(node: ts.Node) {
  if (ts.isCallExpression(node) && node.expression.getText(tree) === "showAlert") {
    alerts.push(node.getText(tree));
  }
  ts.forEachChild(node, visit);
}
visit(tree);

describe("Editor rollback notification contract", () => {
  it("does not interrupt successful writes or rollback with informational dialogs", () => {
    expect(alerts.some((call) => call.includes("editor.rollbackFileReady"))).toBe(false);
    expect(alerts.some((call) => call.includes("editor.transactionRolledBack"))).toBe(false);
  });

  it("retains unknown-outcome warnings, risk review, and rollback file access", () => {
    expect(alerts.some((call) => call.includes("editor.transactionOutcomeUnknown"))).toBe(true);
    expect(source).toContain("await requestRollbackRiskDecision(review)");
    expect(source).toContain("setRollbackFilesByTabId((previous)");
    expect(source).toContain("rollbackFile={rollbackFilesByTabId[activeTab.id]}");
    expect(source).toContain("editor.rollbackFileOpenFailed");
  });
});
