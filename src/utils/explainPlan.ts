import type { Node, Edge } from "@xyflow/react";
import type { ExplainNode, ExplainPlan } from "../types/explain";
import type { ExplainPlanNodeData } from "../components/ui/ExplainPlanNode";
import dagre from "dagre";

// ---------------------------------------------------------------------------
// Tree → ReactFlow conversion
// ---------------------------------------------------------------------------

export function explainPlanToFlow(plan: ExplainPlan): {
  nodes: Node[];
  edges: Edge[];
} {
  const maxCost = getMaxCost(plan.root);
  const maxTime = getMaxTime(plan.root);
  const rawNodes: Node[] = [];
  const edges: Edge[] = [];

  function walk(node: ExplainNode) {
    const data: ExplainPlanNodeData = {
      node,
      maxCost,
      maxTime,
      hasAnalyzeData: plan.has_analyze_data,
    };

    rawNodes.push({
      id: node.id,
      type: "explainPlan",
      position: { x: 0, y: 0 },
      data,
    });

    for (const child of node.children) {
      edges.push({
        id: `${node.id}-${child.id}`,
        source: node.id,
        target: child.id,
        animated: true,
        style: { stroke: "#6366f1" },
      });
      walk(child);
    }
  }

  walk(plan.root);

  return layoutExplainNodes(rawNodes, edges);
}

// ---------------------------------------------------------------------------
// Dagre layout
// ---------------------------------------------------------------------------

export function layoutExplainNodes(
  nodes: Node[],
  edges: Edge[],
): { nodes: Node[]; edges: Edge[] } {
  const g = new dagre.graphlib.Graph();
  g.setDefaultEdgeLabel(() => ({}));
  g.setGraph({ rankdir: "TB", ranksep: 80, nodesep: 40 });

  const NODE_WIDTH = 280;

  for (const node of nodes) {
    const data = node.data as ExplainPlanNodeData;
    const lines = 3 + (data.hasAnalyzeData ? 1 : 0) + (data.node.filter ? 1 : 0);
    const height = 28 + lines * 22;
    g.setNode(node.id, { width: NODE_WIDTH, height });
  }

  for (const edge of edges) {
    g.setEdge(edge.source, edge.target);
  }

  dagre.layout(g);

  const layoutedNodes = nodes.map((node) => {
    const pos = g.node(node.id);
    return {
      ...node,
      position: {
        x: pos.x - NODE_WIDTH / 2,
        y: pos.y - pos.height / 2,
      },
    };
  });

  return { nodes: layoutedNodes, edges };
}

// ---------------------------------------------------------------------------
// Color helpers
// ---------------------------------------------------------------------------

export function getNodeCostColor(cost: number, maxCost: number): string {
  if (maxCost <= 0) return "border-green-500";
  const ratio = cost / maxCost;
  if (ratio < 0.2) return "border-green-500";
  if (ratio < 0.6) return "border-yellow-500";
  return "border-red-500";
}

// ---------------------------------------------------------------------------
// Formatters
// ---------------------------------------------------------------------------

export function formatCost(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  if (n >= 100) return n.toFixed(0);
  if (n >= 1) return n.toFixed(1);
  return n.toFixed(2);
}

export function formatTime(ms: number): string {
  if (ms >= 1000) return `${(ms / 1000).toFixed(2)} s`;
  if (ms >= 1) return `${ms.toFixed(2)} ms`;
  return `${(ms * 1000).toFixed(0)} us`;
}

export function formatRows(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toFixed(0);
}

// ---------------------------------------------------------------------------
// Tree traversal helpers
// ---------------------------------------------------------------------------

export function getMaxCost(node: ExplainNode): number {
  let max = node.total_cost ?? 0;
  for (const child of node.children) {
    const childMax = getMaxCost(child);
    if (childMax > max) max = childMax;
  }
  return max;
}

export function getMaxTime(node: ExplainNode): number {
  let max = node.actual_time_ms ?? 0;
  for (const child of node.children) {
    const childMax = getMaxTime(child);
    if (childMax > max) max = childMax;
  }
  return max;
}

// ---------------------------------------------------------------------------
// Query type detection
// ---------------------------------------------------------------------------

export function isDataModifyingQuery(query: string): boolean {
  const trimmed = query.trim().toUpperCase();
  return (
    trimmed.startsWith("INSERT") ||
    trimmed.startsWith("UPDATE") ||
    trimmed.startsWith("DELETE") ||
    trimmed.startsWith("DROP") ||
    trimmed.startsWith("ALTER") ||
    trimmed.startsWith("TRUNCATE")
  );
}
