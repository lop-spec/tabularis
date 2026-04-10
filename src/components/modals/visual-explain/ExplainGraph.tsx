import { useEffect, useMemo, useCallback } from "react";
import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  useNodesState,
  useEdgesState,
  useReactFlow,
  ReactFlowProvider,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import type { ExplainPlan } from "../../../types/explain";
import { explainPlanToFlow } from "../../../utils/explainPlan";
import { ExplainPlanNodeComponent } from "../../ui/ExplainPlanNode";

const nodeTypes = {
  explainPlan: ExplainPlanNodeComponent,
};

interface ExplainGraphInnerProps {
  plan: ExplainPlan;
}

function ExplainGraphInner({ plan }: ExplainGraphInnerProps) {
  const { fitView } = useReactFlow();

  const { nodes: initialNodes, edges: initialEdges } = useMemo(
    () => explainPlanToFlow(plan),
    [plan],
  );

  const [nodes, setNodes, onNodesChange] = useNodesState(initialNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState(initialEdges);

  useEffect(() => {
    setNodes(initialNodes);
    setEdges(initialEdges);
  }, [initialNodes, initialEdges, setNodes, setEdges]);

  const handleInit = useCallback(() => {
    setTimeout(() => fitView({ padding: 0.2 }), 50);
  }, [fitView]);

  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      onNodesChange={onNodesChange}
      onEdgesChange={onEdgesChange}
      nodeTypes={nodeTypes}
      onInit={handleInit}
      fitView
      proOptions={{ hideAttribution: true }}
      minZoom={0.1}
      maxZoom={2}
    >
      <Background />
      <Controls />
      {nodes.length > 10 && (
        <MiniMap
          nodeStrokeWidth={3}
          className="!bg-base !border-default"
        />
      )}
    </ReactFlow>
  );
}

interface ExplainGraphProps {
  plan: ExplainPlan;
}

export const ExplainGraph = ({ plan }: ExplainGraphProps) => {
  return (
    <div className="h-full w-full">
      <ReactFlowProvider>
        <ExplainGraphInner plan={plan} />
      </ReactFlowProvider>
    </div>
  );
};
