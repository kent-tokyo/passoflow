import { useCallback, useState } from "react"
import type { Edge, Node } from "reactflow"
import type { StepNodeData } from "../types/scenario"

export const MAX_FLOW_HISTORY = 100

export interface FlowSnapshot {
  nodes: Node<StepNodeData>[]
  edges: Edge[]
}

interface Props {
  nodes: Node<StepNodeData>[]
  edges: Edge[]
  setNodes: (nodes: Node<StepNodeData>[]) => void
  setEdges: (edges: Edge[]) => void
  onRestore: () => void
}

export function useFlowHistory({ nodes, edges, setNodes, setEdges, onRestore }: Props) {
  const [past, setPast] = useState<FlowSnapshot[]>([])
  const [future, setFuture] = useState<FlowSnapshot[]>([])

  const pushHistory = useCallback(() => {
    setPast((current) => [...current, { nodes, edges }].slice(-MAX_FLOW_HISTORY))
    setFuture([])
  }, [nodes, edges])

  const undo = useCallback(() => {
    if (past.length === 0) return
    const previous = past[past.length - 1]
    setFuture((current) => [{ nodes, edges }, ...current])
    setPast((current) => current.slice(0, -1))
    setNodes(previous.nodes)
    setEdges(previous.edges)
    onRestore()
  }, [past, nodes, edges, setNodes, setEdges, onRestore])

  const redo = useCallback(() => {
    if (future.length === 0) return
    const next = future[0]
    setPast((current) => [...current, { nodes, edges }].slice(-MAX_FLOW_HISTORY))
    setFuture((current) => current.slice(1))
    setNodes(next.nodes)
    setEdges(next.edges)
    onRestore()
  }, [future, nodes, edges, setNodes, setEdges, onRestore])

  const recordSnapshot = useCallback((snapshot: FlowSnapshot) => {
    setPast((current) => [...current, snapshot].slice(-MAX_FLOW_HISTORY))
    setFuture([])
  }, [])

  const clearHistory = useCallback(() => {
    setPast([])
    setFuture([])
  }, [])

  return { past, future, pushHistory, undo, redo, recordSnapshot, clearHistory }
}
