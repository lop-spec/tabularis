import { memo } from "react";
import { Handle, Position, type Node, type NodeProps } from "@xyflow/react";
import { useTranslation } from "react-i18next";
import type { ExplainNode } from "../../types/explain";
import {
  getNodeCostColor,
  formatCost,
  formatTime,
  formatRows,
} from "../../utils/explainPlan";
import clsx from "clsx";

export interface ExplainPlanNodeData extends Record<string, unknown> {
  node: ExplainNode;
  maxCost: number;
  maxTime: number;
  hasAnalyzeData: boolean;
}

export type ExplainPlanNodeType = Node<ExplainPlanNodeData, "explainPlan">;

export const ExplainPlanNodeComponent = memo(
  ({ data }: NodeProps<ExplainPlanNodeType>) => {
    const { t } = useTranslation();
    const { node, maxCost, hasAnalyzeData } = data;
    const costColor = getNodeCostColor(node.total_cost ?? 0, maxCost);

    return (
      <div
        className={clsx(
          "bg-elevated border border-strong rounded shadow-xl min-w-[260px] max-w-[300px] overflow-hidden",
          "border-l-4",
          costColor,
        )}
      >
        {/* Header */}
        <div className="bg-base px-3 py-2 border-b border-default">
          <div className="text-sm font-bold text-primary">{node.node_type}</div>
          {node.relation && (
            <div className="text-xs text-muted mt-0.5">
              {t("editor.visualExplain.relation")}: {node.relation}
            </div>
          )}
        </div>

        {/* Metrics */}
        <div className="px-3 py-2 space-y-1">
          <div className="flex items-center justify-between text-xs">
            <span className="text-muted">
              {t("editor.visualExplain.estRows")}
            </span>
            <span className="text-secondary font-mono">
              {node.plan_rows != null ? formatRows(node.plan_rows) : "-"}
            </span>
          </div>

          {node.total_cost != null && (
            <div className="flex items-center justify-between text-xs">
              <span className="text-muted">
                {t("editor.visualExplain.cost")}
              </span>
              <span className="text-secondary font-mono">
                {formatCost(node.total_cost)}
              </span>
            </div>
          )}

          {hasAnalyzeData && node.actual_rows != null && (
            <div className="flex items-center justify-between text-xs">
              <span className="text-muted">
                {t("editor.visualExplain.actualRows")}
              </span>
              <span className="text-primary font-mono font-semibold">
                {formatRows(node.actual_rows)}
              </span>
            </div>
          )}

          {hasAnalyzeData && node.actual_time_ms != null && (
            <div className="flex items-center justify-between text-xs">
              <span className="text-muted">
                {t("editor.visualExplain.time")}
              </span>
              <span className="text-primary font-mono font-semibold">
                {formatTime(node.actual_time_ms)}
              </span>
            </div>
          )}

          {hasAnalyzeData && node.actual_loops != null && node.actual_loops > 1 && (
            <div className="flex items-center justify-between text-xs">
              <span className="text-muted">
                {t("editor.visualExplain.loops")}
              </span>
              <span className="text-secondary font-mono">
                {node.actual_loops}
              </span>
            </div>
          )}

          {node.filter && (
            <div className="text-[10px] text-muted mt-1 font-mono truncate border-t border-default/50 pt-1">
              {t("editor.visualExplain.filter")}: {node.filter}
            </div>
          )}

          {node.index_condition && (
            <div className="text-[10px] text-muted font-mono truncate">
              {t("editor.visualExplain.indexCondition")}: {node.index_condition}
            </div>
          )}
        </div>

        <Handle
          type="target"
          position={Position.Top}
          className="!w-2 !h-2 !bg-indigo-500 !border-strong"
        />
        <Handle
          type="source"
          position={Position.Bottom}
          className="!w-2 !h-2 !bg-indigo-500 !border-strong"
        />
      </div>
    );
  },
);
