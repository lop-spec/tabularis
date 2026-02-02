import dagre from 'dagre';

// Position enum values (hardcoded to avoid importing from @xyflow/react in worker)
const Position = {
  Left: 'left',
  Right: 'right',
  Top: 'top',
  Bottom: 'bottom'
};

interface WorkerNode {
  id: string;
  data?: {
    columns?: unknown[];
  };
  [key: string]: unknown;
}

interface WorkerEdge {
  source: string;
  target: string;
  [key: string]: unknown;
}

interface WorkerMessage {
  nodes: WorkerNode[];
  edges: WorkerEdge[];
  direction: string;
}

self.onmessage = (e: MessageEvent<WorkerMessage>) => {
  const { nodes, edges, direction } = e.data;

  const dagreGraph = new dagre.graphlib.Graph();
  dagreGraph.setDefaultEdgeLabel(() => ({}));
  dagreGraph.setGraph({ rankdir: direction, ranksep: 150, nodesep: 50 });

  const nodeWidth = 240;

  nodes.forEach((node: WorkerNode) => {
    const columns = node.data?.columns?.length || 0;
    const height = 40 + (columns * 28);
    dagreGraph.setNode(node.id, { width: nodeWidth, height });
  });

  edges.forEach((edge: WorkerEdge) => {
    dagreGraph.setEdge(edge.source, edge.target);
  });

  dagre.layout(dagreGraph);

  const layoutedNodes = nodes.map((node: WorkerNode) => {
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

  self.postMessage({ nodes: layoutedNodes, edges });
};
