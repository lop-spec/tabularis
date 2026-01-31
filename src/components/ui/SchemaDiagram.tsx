import { useCallback, useEffect, useState, useMemo } from 'react';
import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  useNodesState,
  useEdgesState,
  type Node,
  type Edge,
  Position,
  useReactFlow,
  ReactFlowProvider,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import dagre from 'dagre';
import { useEditor } from '../../hooks/useEditor';
import { SchemaTableNodeComponent } from './SchemaTableNode';
import { Loader2 } from 'lucide-react';

const nodeTypes = {
  schemaTable: SchemaTableNodeComponent,
};

const ANIMATION_THRESHOLD = 50;

const getLayoutedElements = (nodes: Node[], edges: Edge[], direction = 'LR') => {
  const dagreGraph = new dagre.graphlib.Graph();
  dagreGraph.setDefaultEdgeLabel(() => ({}));

  const nodeWidth = 240;
  // Node height approximation unused by dagre simple layout but good for spacing
  // const nodeHeight = 40;

  dagreGraph.setGraph({ rankdir: direction, ranksep: 150, nodesep: 50 });

  nodes.forEach((node) => {
    // Estimate height based on columns for vertical spacing
    const columns = (node.data as any).columns?.length || 0;
    const height = 40 + (columns * 28);
    dagreGraph.setNode(node.id, { width: nodeWidth, height });
  });

  edges.forEach((edge) => {
    dagreGraph.setEdge(edge.source, edge.target);
  });

  dagre.layout(dagreGraph);

  const newNodes = nodes.map((node) => {
    const nodeWithPosition = dagreGraph.node(node.id);
    return {
      ...node,
      targetPosition: direction === 'LR' ? Position.Left : Position.Top,
      sourcePosition: direction === 'LR' ? Position.Right : Position.Bottom,
      position: {
        x: nodeWithPosition.x - nodeWidth / 2,
        y: nodeWithPosition.y - (dagreGraph.node(node.id).height / 2),
      },
    };
  });

  return { nodes: newNodes, edges };
};

interface SchemaDiagramContentProps {
  connectionId: string;
  refreshTrigger: number;
}

const SchemaDiagramContent = ({ connectionId, refreshTrigger }: SchemaDiagramContentProps) => {
  const { getSchema } = useEditor();
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  const { fitView, zoomIn, zoomOut } = useReactFlow();
  const [loading, setLoading] = useState(false);

  // Keyboard shortcuts for zoom
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === '+' || e.key === '=') {
        e.preventDefault();
        zoomIn();
      } else if (e.key === '-' || e.key === '_') {
        e.preventDefault();
        zoomOut();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [zoomIn, zoomOut]);
  // Main effect to load schema from backend
  useEffect(() => {
    let isMounted = true;

    const loadSchema = async () => {
      if (!connectionId) return;
      setLoading(true);

      try {
        const fetchedSchema = await getSchema(connectionId);
        if (!isMounted) return;

        // Build nodes and edges with optimizations
        const initialNodes: Node[] = [];
        const initialEdges: Edge[] = [];
        const tableSet = new Set(fetchedSchema.map(t => t.name));

        fetchedSchema.forEach(table => {
          // Build FK lookup Set ONCE per table for O(1) lookups
          const fkColumnNames = new Set(
            table.foreign_keys.map(fk => fk.column_name)
          );

          initialNodes.push({
            id: table.name,
            type: 'schemaTable',
            position: { x: 0, y: 0 },
            data: {
              label: table.name,
              columns: table.columns.map(c => ({
                name: c.name,
                type: c.data_type,
                isPk: c.is_pk,
                isFk: fkColumnNames.has(c.name) // O(1) lookup
              }))
            }
          });

          table.foreign_keys.forEach(fk => {
            if (tableSet.has(fk.ref_table)) {
              initialEdges.push({
                id: `e-${table.name}-${fk.column_name}-${fk.ref_table}-${fk.ref_column}`,
                source: table.name,
                target: fk.ref_table,
                sourceHandle: fk.column_name,
                targetHandle: fk.ref_column,
                animated: initialEdges.length < ANIMATION_THRESHOLD, // Conditional animation
                style: { stroke: '#6366f1', strokeWidth: 1.5 },
                type: 'smoothstep',
              });
            }
          });
        });

        // Calculate layout
        const { nodes: layoutedNodes, edges: layoutedEdges } = getLayoutedElements(
          initialNodes,
          initialEdges
        );

        if (isMounted) {
          setNodes(layoutedNodes);
          setEdges(layoutedEdges);

          // Fit view after rendering
          setTimeout(() => {
            if (isMounted) {
              fitView({ padding: 0.2 });
            }
          }, 100);
        }

      } catch (e) {
        console.error("Failed to load schema diagram", e);
      } finally {
        if (isMounted) {
          setLoading(false);
        }
      }
    };

    loadSchema();

    return () => {
      isMounted = false;
    };
  }, [connectionId, refreshTrigger, getSchema, fitView, setNodes, setEdges]);

  // Conditionally show MiniMap only for medium-sized schemas
  const shouldShowMiniMap = useMemo(() => {
    const nodeCount = nodes.length;
    return nodeCount >= 10 && nodeCount <= 100;
  }, [nodes.length]);

  return (
    <div className="w-full h-full relative bg-slate-950">
        {loading && (
            <div className="absolute inset-0 z-50 flex items-center justify-center bg-slate-950/80 backdrop-blur-sm">
                <div className="flex flex-col items-center gap-3 text-slate-400">
                    <Loader2 size={32} className="animate-spin text-indigo-500" />
                    <span>Generating Diagram...</span>
                </div>
            </div>
        )}

        <ReactFlow
            nodes={nodes}
            edges={edges}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            nodeTypes={nodeTypes}
            fitView
            minZoom={0.05}
            maxZoom={2}
            defaultEdgeOptions={{ type: 'smoothstep' }}
            nodesDraggable={false}
            nodesConnectable={false}
            elementsSelectable={false}
            panOnScroll={false}
            zoomOnScroll={true}
            zoomOnPinch={true}
            zoomOnDoubleClick={false}
            panOnDrag={true}
        >
            <Background gap={20} size={1} color="#334155" />
            <Controls className="!bg-slate-800 !border-slate-700 !shadow-xl" showInteractive={false} />
            {shouldShowMiniMap && (
                <MiniMap
                    nodeColor={() => '#6366f1'}
                    maskColor="rgba(15, 23, 42, 0.9)"
                    className="!bg-slate-900 !border !border-slate-800 !shadow-xl"
                    style={{ height: 120, width: 200 }}
                />
            )}
        </ReactFlow>
    </div>
  );
};

interface SchemaDiagramProps {
  connectionId: string;
  refreshTrigger: number;
}

export const SchemaDiagram = ({ connectionId, refreshTrigger }: SchemaDiagramProps) => (
    <ReactFlowProvider>
        <SchemaDiagramContent connectionId={connectionId} refreshTrigger={refreshTrigger} />
    </ReactFlowProvider>
);
