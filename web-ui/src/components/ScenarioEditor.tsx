import axios from "axios"
import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties, type DragEvent, type MouseEvent as ReactMouseEvent } from "react"
import {
  addEdge,
  MarkerType,
  useEdgesState,
  useNodesState,
  type Connection,
  type Edge,
  type EdgeChange,
  type Node,
  type NodeChange,
  type ReactFlowInstance,
  type XYPosition,
} from "reactflow"
import "reactflow/dist/style.css"

import {
  createScenario,
  fetchActionSchemas,
  fetchScenario,
  fetchScenarioList,
  fetchVersion,
  pickScenarioSaveFile,
  saveScenario,
  type FlowAction,
} from "../api/scenarioApi"
import { RunningNodeProvider } from "../context/RunningNodeContext"
import { ActionLabelProvider } from "../i18n/ActionLabelContext"
import { useLocale } from "../i18n/useLocale"
import { useFlowHistory, type FlowSnapshot } from "../hooks/useFlowHistory"
import { PALETTE_WIDTH, PARAMETER_PANEL_WIDTH, usePanelLayout } from "../hooks/usePanelLayout"
import { useScenarioShortcuts } from "../hooks/useScenarioShortcuts"
import { useScenarioExecution } from "../hooks/useScenarioExecution"
import { readStorage } from "../lib/storage"
import { recordRecoveryStarted } from "../lib/usageMetrics"
import {
  alignNodesVertically,
  computeIfFrames,
  flowToSteps,
  listTables,
  listVariables,
  nodeTypeForAction,
  orderNodes,
  shiftReachableNodesDown,
  stepsToFlow,
} from "../lib/flowConversion"
import type { ActionSchema, ScenarioSummary, StepNodeData } from "../types/scenario"
import ActionPalette from "./ActionPalette"
import ExecutionPanel, { type RunContext } from "./ExecutionPanel"
import FlowCanvas from "./FlowCanvas"
import FirstUseGuide from "./FirstUseGuide"
import type { GroupFrameData } from "./GroupFrameNode"
import type { IfFrameData } from "./IfFrameNode"
import type { LoopFrameData } from "./LoopFrameNode"
import MenuBar from "./MenuBar"
import ParameterPanel from "./ParameterPanel"
import PanelResizeHandle from "./PanelResizeHandle"
import StatusBar from "./StatusBar"
import type { TableImportResult } from "./TableImportModal"
import TitleBar from "./TitleBar"

const GROUP_PADDING = 28
const GROUP_LABEL_SPACE = 24
const NODE_BOX_WIDTH = 210
const NODE_BOX_HEIGHT = 66
const IF_NODE_GAP = 70
const FRAME_NODE_TYPES = new Set(["groupFrameNode", "loopFrameNode", "ifFrameNode"])
let nextNodeId = 1000

interface ContextMenuState {
  x: number
  y: number
  nodeId?: string
  edgeId?: string
  groupLabel?: string
  loopLabel?: string
  ifId?: string
}

export default function ScenarioEditor() {
  const { locale, t } = useLocale()
  const [schemas, setSchemas] = useState<ActionSchema[]>([])
  const [scenarioSummaries, setScenarioSummaries] = useState<ScenarioSummary[]>([])
  const [currentFile, setCurrentFile] = useState<string>("")
  const [title, setTitle] = useState("")
  const [nodes, setNodes, onNodesChange] = useNodesState<StepNodeData>([])
  const [edges, setEdges, onEdgesChange] = useEdgesState([])
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null)
  const [selectedNodeIds, setSelectedNodeIds] = useState<string[]>([])
  const [editingGroupLabel, setEditingGroupLabel] = useState<string | null>(null)
  const [editingLoopLabel, setEditingLoopLabel] = useState<string | null>(null)
  const [dragOverEdgeId, setDragOverEdgeId] = useState<string | null>(null)
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null)
  const [aiSuggestNodeId, setAiSuggestNodeId] = useState<string | null>(null)
  const [showTableImport, setShowTableImport] = useState(false)
  const [focusMode, setFocusMode] = useState(false)
  const { paletteWidth, setPaletteWidth, parameterPanelWidth, setParameterPanelWidth } = usePanelLayout()
  const toggleFocusMode = useCallback(() => {
    setFocusMode((current) => {
      const next = !current
      setStatusMessage(t(next ? "focusModeEnabled" : "focusModeDisabled"))
      return next
    })
  }, [t])
  const [importedTable, setImportedTable] = useState<{
    name: string
    columns: string[]
    rows: Record<string, string>[]
    selectedRows: number[]
  } | null>(
    null,
  )
  const [logs, setLogs] = useState<string[]>([])
  const [running, setRunning] = useState(false)
  const [runningStep, setRunningStep] = useState<number | null>(null)
  const [statusMessage, setStatusMessage] = useState("")
  const actionLabels = useMemo(
    () => Object.fromEntries(schemas.map((schema) => [schema.action, schema.labels[locale]])),
    [schemas, locale],
  )
  const [version, setVersion] = useState("")
  const [savedSnapshot, setSavedSnapshot] = useState<string | null>(null)
  const [runContext, setRunContext] = useState<RunContext | null>(() => {
    try {
      const stored = readStorage("passoflow-last-run-context")
      return stored ? (JSON.parse(stored) as RunContext) : null
    } catch {
      return null
    }
  })

  // Always mirror the latest currentFile/running, so a callback scheduled earlier (e.g. the
  // "run this step only" countdown) can tell whether the active scenario or run state changed
  // since — the countdown's own closure otherwise only sees whatever these were at click time.
  const currentFileRef = useRef(currentFile)
  useEffect(() => {
    currentFileRef.current = currentFile
  }, [currentFile])
  const runningRef = useRef(running)
  useEffect(() => {
    runningRef.current = running
  }, [running])
  const pendingRunTimeoutRef = useRef<number | null>(null)
  useEffect(() => {
    return () => {
      if (pendingRunTimeoutRef.current !== null) window.clearTimeout(pendingRunTimeoutRef.current)
    }
  }, [])

  const reactFlowWrapper = useRef<HTMLDivElement>(null)
  const [reactFlowInstance, setReactFlowInstance] = useState<ReactFlowInstance | null>(null)
  const nodeDragStartPositionRef = useRef<XYPosition | null>(null)

  const isDraggingRef = useRef(false)
  const paramEditBaseRef = useRef<FlowSnapshot | null>(null)
  const onHistoryRestore = useCallback(() => {
    setSelectedNodeId(null)
    setEditingGroupLabel(null)
    setEditingLoopLabel(null)
  }, [])
  const { past, future, pushHistory, undo, redo, recordSnapshot, clearHistory } = useFlowHistory({
    nodes,
    edges,
    setNodes,
    setEdges,
    onRestore: onHistoryRestore,
  })

  const refreshScenarioList = useCallback(async () => {
    try {
      const summaries = await fetchScenarioList()
      setScenarioSummaries(summaries)
      setCurrentFile((prev) => prev || summaries[0]?.filename || "")
    } catch {
      setStatusMessage(t("fetchScenariosFailed"))
    }
  }, [t])

  useEffect(() => {
    fetchActionSchemas()
      .then(setSchemas)
      .catch(() => setStatusMessage(t("fetchActionsFailed")))
    void refreshScenarioList()
    fetchVersion().then(setVersion).catch(() => {})
  }, [t, refreshScenarioList])

  useEffect(() => {
    if (!currentFile) {
      setNodes([])
      setEdges([])
      setImportedTable(null)
      setTitle("")
      setSavedSnapshot(null)
      setSelectedNodeId(null)
      setSelectedNodeIds([])
      clearHistory()
      isDraggingRef.current = false
      paramEditBaseRef.current = null
      return
    }
    setStatusMessage(t("loadingScenario"))
    setSavedSnapshot(null)
    setImportedTable(null)
    let cancelled = false
    fetchScenario(currentFile)
      .then((data) => {
        if (cancelled) return
        const { nodes: newNodes, edges: newEdges } = stepsToFlow(data.steps)
        setNodes(newNodes)
        setEdges(newEdges)
        setTitle(data.title)
        setSavedSnapshot(JSON.stringify({ title: data.title, steps: flowToSteps(newNodes, newEdges) }))
        setSelectedNodeId(null)
        clearHistory()
        isDraggingRef.current = false
        paramEditBaseRef.current = null
      })
      .catch(() => { if (!cancelled) setStatusMessage(t("loadScenarioFailed", { file: currentFile })) })
    return () => {
      cancelled = true
    }
  }, [clearHistory, currentFile, setNodes, setEdges, t])

  const onConnect = useCallback(
    (connection: Connection) => {
      pushHistory()
      setEdges((eds) => addEdge(connection, eds))
    },
    [pushHistory, setEdges],
  )

  const handleNodesChange = useCallback(
    (changes: NodeChange[]) => {
      for (const change of changes) {
        if (change.type === "remove") {
          pushHistory()
          break
        }
        if (change.type === "position" && change.dragging && !isDraggingRef.current) {
          isDraggingRef.current = true
          pushHistory()
        } else if (change.type === "position" && change.dragging === false) {
          isDraggingRef.current = false
        }
      }

      // Dragging one member of a group drags the whole group together: for each node being
      // dragged that belongs to a group, apply the same position delta to every other member
      // not already part of this change batch (e.g. a multi-selected drag already moves those).
      // handledIds grows as extra changes are queued, so a group with 2+ already-dragged
      // members (e.g. a multi-select drag) doesn't queue the same sibling twice.
      const handledIds = new Set(
        changes
          .filter(
            (c): c is Extract<NodeChange, { type: "dimensions" | "position" | "select" | "remove" }> =>
              c.type === "dimensions" || c.type === "position" || c.type === "select" || c.type === "remove",
          )
          .map((c) => c.id),
      )
      const extraChanges: NodeChange[] = []
      for (const change of changes) {
        if (change.type !== "position") continue
        const node = nodes.find((n) => n.id === change.id)
        const group = node?.data.group
        if (!node || !group) continue

        if (change.dragging && change.position) {
          const dx = change.position.x - node.position.x
          const dy = change.position.y - node.position.y
          if (dx === 0 && dy === 0) continue
          for (const sibling of nodes) {
            if (sibling.data.group === group && !handledIds.has(sibling.id)) {
              handledIds.add(sibling.id)
              extraChanges.push({
                id: sibling.id,
                type: "position",
                dragging: true,
                position: { x: sibling.position.x + dx, y: sibling.position.y + dy },
              })
            }
          }
        } else if (change.dragging === false) {
          // The drag just ended: release siblings' "dragging" flag too (their position was
          // already finalized by the last intermediate update above) — otherwise a group
          // member never directly under the cursor stays stuck mid-drag internally, even
          // though it visually stopped moving.
          for (const sibling of nodes) {
            if (sibling.data.group === group && !handledIds.has(sibling.id)) {
              handledIds.add(sibling.id)
              extraChanges.push({
                id: sibling.id,
                type: "position",
                dragging: false,
                position: sibling.position,
              })
            }
          }
        }
      }

      onNodesChange(extraChanges.length ? [...changes, ...extraChanges] : changes)
    },
    [onNodesChange, pushHistory, nodes],
  )

  const handleEdgesChange = useCallback(
    (changes: EdgeChange[]) => {
      if (changes.some((c) => c.type === "remove")) pushHistory()
      onEdgesChange(changes)
    },
    [onEdgesChange, pushHistory],
  )

  const onNodeClick = useCallback((_event: unknown, node: Node) => {
    setContextMenu(null)
    if (node.type && FRAME_NODE_TYPES.has(node.type)) return
    setSelectedNodeId(node.id)
    setStatusMessage(t("nodeSelected", { action: actionLabels[node.data.action] ?? node.data.action }))
  }, [actionLabels, t])

  const onSelectionChange = useCallback(({ nodes: selected }: { nodes: Node[] }) => {
    setSelectedNodeIds(selected.filter((n) => !n.type || !FRAME_NODE_TYPES.has(n.type)).map((n) => n.id))
  }, [])

  const onPaneClick = useCallback(() => {
    setSelectedNodeId(null)
    setContextMenu(null)
  }, [])

  // Edges render as thin bezier strokes, so requiring the cursor to land exactly on the
  // rendered path (or even React Flow's own invisible ~20px interaction path) makes dropping
  // onto a short connector between two nodes fiddly — the adjacent node's own DOM element
  // covers most of the nearby space and wins elementFromPoint first. As a fallback, treat a
  // drop as "on edge X" if it's within EDGE_DROP_TOLERANCE px of the straight line between
  // that edge's source and target node centers (but not actually inside a node's own rect,
  // so dropping directly on a node still behaves as before).
  const EDGE_DROP_TOLERANCE = 30

  const isInsideAnyNode = (clientX: number, clientY: number): boolean => {
    for (const el of document.querySelectorAll(".react-flow__node")) {
      const r = el.getBoundingClientRect()
      if (clientX >= r.left && clientX <= r.right && clientY >= r.top && clientY <= r.bottom) return true
    }
    return false
  }

  const distanceToSegment = (px: number, py: number, x1: number, y1: number, x2: number, y2: number): number => {
    const dx = x2 - x1
    const dy = y2 - y1
    const lengthSq = dx * dx + dy * dy
    const t = lengthSq === 0 ? 0 : Math.max(0, Math.min(1, ((px - x1) * dx + (py - y1) * dy) / lengthSq))
    return Math.hypot(px - (x1 + t * dx), py - (y1 + t * dy))
  }

  const nearestEdgeNear = useCallback(
    (clientX: number, clientY: number): string | null => {
      if (isInsideAnyNode(clientX, clientY)) return null
      let best: { id: string; dist: number } | null = null
      for (const edge of edges) {
        const sourceEl = document.querySelector(`.react-flow__node[data-id="${CSS.escape(edge.source)}"]`)
        const targetEl = document.querySelector(`.react-flow__node[data-id="${CSS.escape(edge.target)}"]`)
        if (!sourceEl || !targetEl) continue
        const s = sourceEl.getBoundingClientRect()
        const t = targetEl.getBoundingClientRect()
        const dist = distanceToSegment(
          clientX,
          clientY,
          s.left + s.width / 2,
          s.top + s.height / 2,
          t.left + t.width / 2,
          t.top + t.height / 2,
        )
        if (dist <= EDGE_DROP_TOLERANCE && (!best || dist < best.dist)) {
          best = { id: edge.id, dist }
        }
      }
      return best?.id ?? null
    },
    [edges],
  )

  const edgeIdAtPoint = useCallback(
    (clientX: number, clientY: number): string | null => {
      const el = document.elementFromPoint(clientX, clientY)
      const testId = el?.closest("[data-testid^='rf__edge-']")?.getAttribute("data-testid")
      if (testId) return testId.replace("rf__edge-", "")
      return nearestEdgeNear(clientX, clientY)
    },
    [nearestEdgeNear],
  )

  const onDragOver = useCallback(
    (event: DragEvent) => {
      event.preventDefault()
      event.dataTransfer.dropEffect = "move"
      setDragOverEdgeId(edgeIdAtPoint(event.clientX, event.clientY))
    },
    [edgeIdAtPoint],
  )

  const onDragLeave = useCallback(() => setDragOverEdgeId(null), [])

  const onNodeDragStart = useCallback((_event: ReactMouseEvent, node: Node) => {
    nodeDragStartPositionRef.current = node.position
  }, [])

  // Dropping an existing node directly onto a different connecting line moves it to that
  // point in the sequence: its old edges are bypassed (predecessor reconnected straight to
  // successor) and it's spliced into the target edge instead, the same way a brand-new
  // action from the palette gets inserted onto a line. A small movement threshold avoids
  // rewiring on an accidental micro-jiggle that happens to land on a nearby line.
  const onNodeDragStop = useCallback(
    (event: ReactMouseEvent, node: Node) => {
      const start = nodeDragStartPositionRef.current
      nodeDragStartPositionRef.current = null
      if (node.type && FRAME_NODE_TYPES.has(node.type)) return
      if (start && Math.hypot(node.position.x - start.x, node.position.y - start.y) < 10) return

      const targetEdgeId = edgeIdAtPoint(event.clientX, event.clientY)
      if (!targetEdgeId) return
      const targetEdge = edges.find((e) => e.id === targetEdgeId)
      if (!targetEdge || targetEdge.source === node.id || targetEdge.target === node.id) return

      setEdges((eds) => {
        const inEdge = eds.find((e) => e.target === node.id)
        const outEdge = eds.find((e) => e.source === node.id)
        let next = eds.filter((e) => e.source !== node.id && e.target !== node.id && e.id !== targetEdge.id)
        if (inEdge && outEdge) {
          next = next.concat({ id: `e${inEdge.source}-${outEdge.target}`, source: inEdge.source, target: outEdge.target })
        }
        return next.concat(
          { id: `e${targetEdge.source}-${node.id}`, source: targetEdge.source, target: node.id },
          { id: `e${node.id}-${targetEdge.target}`, source: node.id, target: targetEdge.target },
        )
      })
    },
    [edgeIdAtPoint, edges, setEdges],
  )

  const addNode = useCallback(
    (action: string, position: XYPosition) => {
      pushHistory()
      const id = String(nextNodeId++)
      const newNode: Node<StepNodeData> = {
        id,
        type: nodeTypeForAction(action),
        position,
        data: { action, params: {} },
        selected: true,
      }
      setNodes((nds) => nds.map<Node<StepNodeData>>((node) => ({ ...node, selected: false })).concat(newNode))
      setSelectedNodeId(id)
      setSelectedNodeIds([id])
      return id
    },
    [pushHistory, setNodes],
  )

  const insertNodeOnEdge = useCallback(
    (action: string, edge: Edge, position: XYPosition) => {
      pushHistory()
      const id = String(nextNodeId++)
      const newNode: Node<StepNodeData> = {
        id,
        type: nodeTypeForAction(action),
        position,
        data: { action, params: {} },
        selected: true,
      }
      setNodes((nds) => nds.map<Node<StepNodeData>>((node) => ({ ...node, selected: false })).concat(newNode))
      setEdges((eds) => [
        ...eds.filter((e) => e.id !== edge.id),
        { id: `e${edge.source}-${id}`, source: edge.source, target: id },
        { id: `e${id}-${edge.target}`, source: id, target: edge.target },
      ])
      setSelectedNodeId(id)
      setSelectedNodeIds([id])
      return id
    },
    [pushHistory, setNodes, setEdges],
  )

  const insertNodeBefore = useCallback(
    (action: string, targetId: string, position: XYPosition) => {
      const targetEdge = edges.find((edge) => edge.target === targetId)
      if (!targetEdge) return false
      pushHistory()
      const id = String(nextNodeId++)
      const newNode: Node<StepNodeData> = {
        id,
        type: nodeTypeForAction(action),
        position,
        data: { action, params: {} },
        selected: true,
      }
      setNodes((nds) => nds.map<Node<StepNodeData>>((node) => ({ ...node, selected: false })).concat(newNode))
      setEdges((eds) => [
        ...eds.filter((edge) => edge.id !== targetEdge.id),
        { id: `e${targetEdge.source}-${id}`, source: targetEdge.source, target: id },
        { id: `e${id}-${targetId}`, source: id, target: targetId },
      ])
      setSelectedNodeId(id)
      setSelectedNodeIds([id])
      return id
    },
    [edges, pushHistory, setNodes, setEdges],
  )

  const addActionFromPalette = useCallback(
    (action: string) => {
      const selected = selectedNodeId ? nodes.find((node) => node.id === selectedNodeId) : undefined
      const target = selected?.data.action === "end"
        ? selected
        : selected
          ? nodes.find((node) => edges.some((edge) => edge.source === selected.id && edge.target === node.id))
          : nodes.find((node) => node.data.action === "end")
      if (target && insertNodeBefore(action, target.id, { x: target.position.x, y: target.position.y - IF_NODE_GAP })) {
        setStatusMessage(t("actionAdded", { action: actionLabels[action] ?? action }))
        return
      }
      addNode(action, { x: 250, y: 140 })
      setStatusMessage(t("actionAdded", { action: actionLabels[action] ?? action }))
    },
    [actionLabels, addNode, edges, insertNodeBefore, nodes, selectedNodeId, t],
  )

  const ifBranchTargetAt = useCallback(
    (position: XYPosition): string | null => {
      const ordered = orderNodes(nodes, edges)
      const nodeById = new Map(nodes.map((node) => [node.id, node]))
      for (const frame of computeIfFrames(nodes, edges)) {
        if (!frame.elseId) continue
        const start = ordered.findIndex((node) => node.id === frame.ifId)
        const end = ordered.findIndex((node) => node.id === frame.endifId)
        if (start < 0 || end < start) continue
        const members = ordered.slice(start, end + 1)
        const minX = Math.min(...members.map((node) => node.position.x))
        const minY = Math.min(...members.map((node) => node.position.y))
        const maxX = Math.max(...members.map((node) => node.position.x + NODE_BOX_WIDTH))
        const maxY = Math.max(...members.map((node) => node.position.y + NODE_BOX_HEIGHT))
        const frameLeft = minX - GROUP_PADDING
        const frameTop = minY - GROUP_PADDING - GROUP_LABEL_SPACE
        const frameRight = maxX + GROUP_PADDING
        const frameBottom = maxY + GROUP_PADDING
        if (position.x < frameLeft || position.x > frameRight || position.y < frameTop || position.y > frameBottom) {
          continue
        }
        const elseNode = nodeById.get(frame.elseId)
        if (!elseNode) continue
        const divider = elseNode.position.y - IF_NODE_GAP / 2
        return position.y < divider ? frame.elseId : frame.endifId
      }
      return null
    },
    [nodes, edges],
  )

  const screenToFlowPosition = useCallback(
    (screenX: number, screenY: number): XYPosition | null => {
      if (!reactFlowWrapper.current || !reactFlowInstance) return null
      const bounds = reactFlowWrapper.current.getBoundingClientRect()
      return reactFlowInstance.screenToFlowPosition({
        x: screenX - bounds.left,
        y: screenY - bounds.top,
      })
    },
    [reactFlowInstance],
  )

  const onDrop = useCallback(
    (event: DragEvent) => {
      event.preventDefault()
      const action = event.dataTransfer.getData("application/passoflow-action")
      const edgeId = dragOverEdgeId
      setDragOverEdgeId(null)
      if (!action) return
      const position = screenToFlowPosition(event.clientX, event.clientY)
      if (!position) return
      const branchTarget = ifBranchTargetAt(position)
      if (branchTarget && insertNodeBefore(action, branchTarget, position)) return
      const edge = edgeId ? edges.find((e) => e.id === edgeId) : undefined
      if (edge) {
        insertNodeOnEdge(action, edge, position)
      } else {
        addNode(action, position)
      }
    },
    [addNode, insertNodeBefore, insertNodeOnEdge, screenToFlowPosition, edges, dragOverEdgeId, ifBranchTargetAt],
  )

  // Holds the last-copied node's action/params, independent of React state — copying doesn't
  // itself trigger a re-render or history entry, only pasting does.
  // Holds the last-copied node(s)' action/params, independent of React state — copying doesn't
  // itself trigger a re-render or history entry, only pasting/cutting does. dx/dy are each
  // entry's offset from the selection's top-left corner, so a multi-node paste keeps their
  // relative layout.
  const clipboardRef = useRef<{
    nodes: { action: string; params: Record<string, unknown>; dx: number; dy: number }[]
  } | null>(null)
  const lastMousePositionRef = useRef<{ x: number; y: number } | null>(null)
  const handleCanvasMouseMove = useCallback((event: ReactMouseEvent<HTMLDivElement>) => {
    lastMousePositionRef.current = { x: event.clientX, y: event.clientY }
  }, [])

  const copySelectedNodes = useCallback(() => {
    const ids = selectedNodeIds.length > 0 ? selectedNodeIds : selectedNodeId ? [selectedNodeId] : []
    const selected = nodes.filter((n) => ids.includes(n.id))
    if (selected.length === 0) return
    const originX = Math.min(...selected.map((n) => n.position.x))
    const originY = Math.min(...selected.map((n) => n.position.y))
    clipboardRef.current = {
      nodes: selected.map((n) => ({
        action: n.data.action,
        params: { ...n.data.params },
        dx: n.position.x - originX,
        dy: n.position.y - originY,
      })),
    }
  }, [nodes, selectedNodeIds, selectedNodeId])

  // Right-click "Copy" on a node that's part of a larger multi-selection copies the whole
  // selection (matching Ctrl+C's behavior via copySelectedNodes), not just the one right-clicked.
  const copyNode = useCallback(
    (nodeId: string) => {
      if (selectedNodeIds.length >= 2 && selectedNodeIds.includes(nodeId)) {
        copySelectedNodes()
        return
      }
      const source = nodes.find((n) => n.id === nodeId)
      if (!source) return
      clipboardRef.current = { nodes: [{ action: source.data.action, params: { ...source.data.params }, dx: 0, dy: 0 }] }
    },
    [nodes, selectedNodeIds, copySelectedNodes],
  )

  const pasteNodesAt = useCallback(
    (position: XYPosition) => {
      const clipboard = clipboardRef.current
      if (!clipboard || clipboard.nodes.length === 0) return
      pushHistory()
      const newNodes: Node<StepNodeData>[] = clipboard.nodes.map((entry) => ({
        id: String(nextNodeId++),
        type: nodeTypeForAction(entry.action),
        position: { x: position.x + entry.dx, y: position.y + entry.dy },
        data: { action: entry.action, params: { ...entry.params } },
      }))
      setNodes((nds) => nds.concat(newNodes))
    },
    [pushHistory, setNodes],
  )

  // Importing a table adds/updates its internal load_table step and wraps Start..End in a table loop.
  const handleTableImport = useCallback(
    (result: TableImportResult) => {
      const selectedRows = result.rows.map((_, index) => index)
      const ordered = orderNodes(nodes, edges)
      const startNode = ordered.find((node) => node.data.action === "start")
      const endNode = ordered.find((node) => node.data.action === "end")
      if (!startNode || !endNode) {
        setStatusMessage(t("tableImportNeedsStartEnd"))
        setImportedTable({ name: result.name, columns: result.columns, rows: result.rows, selectedRows })
        return
      }

      const startIndex = ordered.findIndex((node) => node.id === startNode.id)
      const endIndex = ordered.findIndex((node) => node.id === endNode.id)
      const bodyIds = new Set(ordered.slice(startIndex, endIndex + 1).map((node) => node.id))
      const existingLoad = nodes.find(
        (node) => node.data.action === "load_table" && node.data.params.name === result.name,
      )
      const existingLabels = new Set(nodes.map((node) => node.data.loop).filter(Boolean))
      let loopLabel = result.name
      let suffix = 2
      while (existingLabels.has(loopLabel)) loopLabel = `${result.name}${suffix++}`

      pushHistory()
      const importedParams = {
        ...(existingLoad?.data.params ?? {}),
        path: result.path,
        sheet: result.sheet || undefined,
        name: result.name,
        columns: result.columns,
        selected_rows: selectedRows,
      }
      let nextNodes = nodes.map((node) =>
        bodyIds.has(node.id)
          ? { ...node, data: { ...node.data, loop: loopLabel, loopCount: undefined, loopTable: result.name } }
          : node,
      )
      if (existingLoad) {
        nextNodes = nextNodes.map((node) =>
          node.id === existingLoad.id ? { ...node, data: { ...node.data, params: importedParams } } : node,
        )
      } else {
        const loadId = String(nextNodeId++)
        const loadNode: Node<StepNodeData> = {
          id: loadId,
          type: "actionNode",
          position: { x: startNode.position.x, y: startNode.position.y - IF_NODE_GAP * 2 },
          data: { action: "load_table", params: importedParams },
        }
        nextNodes = nextNodes.concat(loadNode)
        setEdges((eds) => {
          const inEdge = eds.find((edge) => edge.target === startNode.id)
          if (!inEdge) return eds.concat({ id: `e${loadId}-${startNode.id}`, source: loadId, target: startNode.id })
          return eds
            .filter((edge) => edge.id !== inEdge.id)
            .concat(
              { id: `e${inEdge.source}-${loadId}`, source: inEdge.source, target: loadId },
              { id: `e${loadId}-${startNode.id}`, source: loadId, target: startNode.id },
            )
        })
      }
      setNodes(nextNodes)
      setImportedTable({ name: result.name, columns: result.columns, rows: result.rows, selectedRows })
    },
    [nodes, edges, pushHistory, setNodes, setEdges, t],
  )

  const handleImportedRowsChange = useCallback(
    (selectedRows: number[]) => {
      setImportedTable((current) => (current ? { ...current, selectedRows } : current))
      if (!importedTable) return
      setNodes((nds) =>
        nds.map((node) =>
          node.data.action === "load_table" && node.data.params.name === importedTable.name
            ? { ...node, data: { ...node.data, params: { ...node.data.params, selected_rows: selectedRows } } }
            : node,
        ),
      )
    },
    [importedTable, setNodes],
  )

  const cutSelectedNodes = useCallback(() => {
    const ids = selectedNodeIds.length > 0 ? selectedNodeIds : selectedNodeId ? [selectedNodeId] : []
    if (ids.length === 0) return
    copySelectedNodes()
    pushHistory()
    const idsToRemove = new Set(ids)
    setNodes((nds) => nds.filter((n) => !idsToRemove.has(n.id)))
    setEdges((eds) => eds.filter((e) => !idsToRemove.has(e.source) && !idsToRemove.has(e.target)))
    setSelectedNodeId(null)
  }, [selectedNodeIds, selectedNodeId, copySelectedNodes, pushHistory, setNodes, setEdges])

  const openNodeContextMenu = useCallback(
    (x: number, y: number, node: Node) => {
      if (node.type === "groupFrameNode") {
        setContextMenu({ x, y, groupLabel: (node.data as GroupFrameData).label })
        return
      }
      if (node.type === "loopFrameNode") {
        setContextMenu({ x, y, loopLabel: (node.data as LoopFrameData).label })
        return
      }
      if (node.type === "ifFrameNode") {
        setContextMenu({ x, y, ifId: (node.data as IfFrameData).ifId })
        return
      }
      if (!selectedNodeIds.includes(node.id)) {
        setSelectedNodeId(node.id)
      }
      setContextMenu({ x, y, nodeId: node.id })
    },
    [selectedNodeIds],
  )

  const onNodeContextMenu = useCallback(
    (event: ReactMouseEvent, node: Node) => {
      event.preventDefault()
      openNodeContextMenu(event.clientX, event.clientY, node)
    },
    [openNodeContextMenu],
  )

  const nudgeNode = useCallback(
    (nodeId: string, dx: number, dy: number) => {
      const node = nodes.find((candidate) => candidate.id === nodeId)
      if (!node || node.data.action === "start" || node.data.action === "end") return
      pushHistory()
      setNodes((nds) => nds.map((candidate) =>
        candidate.id === nodeId
          ? { ...candidate, position: { x: candidate.position.x + dx, y: candidate.position.y + dy } }
          : candidate,
      ))
      setSelectedNodeId(nodeId)
      setSelectedNodeIds([nodeId])
      setStatusMessage(t("nodeMoved", { action: actionLabels[node.data.action] ?? node.data.action }))
    },
    [actionLabels, nodes, pushHistory, setNodes, t],
  )

  const focusAdjacentNode = useCallback(
    (nodeId: string, direction: "left" | "right" | "up" | "down") => {
      const visibleNodes = nodes.filter((node) => node.data.action !== "load_table")
      const current = visibleNodes.find((node) => node.id === nodeId)
      if (!current) return
      const currentCenter = {
        x: current.position.x + NODE_BOX_WIDTH / 2,
        y: current.position.y + NODE_BOX_HEIGHT / 2,
      }
      const candidates = visibleNodes
        .filter((node) => node.id !== nodeId)
        .map((node) => {
          const center = { x: node.position.x + NODE_BOX_WIDTH / 2, y: node.position.y + NODE_BOX_HEIGHT / 2 }
          const dx = center.x - currentCenter.x
          const dy = center.y - currentCenter.y
          const inDirection = direction === "left" ? dx < 0 : direction === "right" ? dx > 0 : direction === "up" ? dy < 0 : dy > 0
          if (!inDirection) return null
          const primary = direction === "left" || direction === "right" ? Math.abs(dx) : Math.abs(dy)
          const secondary = direction === "left" || direction === "right" ? Math.abs(dy) : Math.abs(dx)
          return { node, score: primary + secondary * 2 }
        })
        .filter((candidate): candidate is { node: Node<StepNodeData>; score: number } => candidate !== null)
        .sort((a, b) => a.score - b.score)
      const next = candidates[0]?.node
      if (!next) return
      setSelectedNodeId(next.id)
      setSelectedNodeIds([next.id])
      requestAnimationFrame(() => {
        const element = Array.from(document.querySelectorAll<HTMLElement>(".react-flow__node"))
          .find((candidate) => candidate.dataset.id === next.id)
        element?.querySelector<HTMLElement>("[role='button'], input, select, textarea")?.focus()
      })
    },
    [nodes],
  )

  const onEdgeContextMenu = useCallback((event: ReactMouseEvent, edge: Edge) => {
    event.preventDefault()
    setContextMenu({ x: event.clientX, y: event.clientY, edgeId: edge.id })
  }, [])

  const onPaneContextMenu = useCallback((event: ReactMouseEvent) => {
    event.preventDefault()
    setContextMenu({ x: event.clientX, y: event.clientY })
  }, [])

  const closeContextMenu = useCallback(() => setContextMenu(null), [])

  const updateNodeParams = useCallback(
    (nodeId: string, params: Record<string, unknown>) => {
      if (!paramEditBaseRef.current) {
        paramEditBaseRef.current = { nodes, edges }
      }
      setNodes((nds) => nds.map((n) => (n.id === nodeId ? { ...n, data: { ...n.data, params } } : n)))
    },
    [nodes, edges, setNodes],
  )

  const commitParamEdit = useCallback(() => {
    if (!paramEditBaseRef.current) return
    recordSnapshot(paramEditBaseRef.current as FlowSnapshot)
    paramEditBaseRef.current = null
  }, [recordSnapshot])

  const deleteNode = useCallback(
    (nodeId: string) => {
      const node = nodes.find((candidate) => candidate.id === nodeId)
      if (!node) return
      if (node.data.action === "start" || node.data.action === "end") {
        setStatusMessage(t("cannotDeleteBoundaryNode"))
        return
      }
      if (!window.confirm(t("deleteNodeConfirm", { action: actionLabels[node.data.action] ?? node.data.action }))) return
      pushHistory()
      setNodes((nds) => nds.filter((n) => n.id !== nodeId))
      setEdges((eds) => {
        const incoming = eds.filter((edge) => edge.target === nodeId)
        const outgoing = eds.filter((edge) => edge.source === nodeId)
        const remaining = eds.filter((edge) => edge.source !== nodeId && edge.target !== nodeId)
        if (incoming.length === 1 && outgoing.length === 1) {
          remaining.push({
            id: `e${incoming[0].source}-${outgoing[0].target}`,
            source: incoming[0].source,
            target: outgoing[0].target,
          })
        }
        return remaining
      })
      setSelectedNodeId(null)
    },
    [actionLabels, nodes, pushHistory, setNodes, setEdges, setStatusMessage, t],
  )

  const deleteEdge = useCallback(
    (edgeId: string) => {
      pushHistory()
      setEdges((eds) => eds.filter((e) => e.id !== edgeId))
    },
    [pushHistory, setEdges],
  )

  const handleAlign = useCallback(() => {
    pushHistory()
    setNodes((nds) => alignNodesVertically(nds, edges))
  }, [edges, pushHistory, setNodes])

  const isConsecutiveSelection = useCallback(
    (nodeIds: string[]) => {
      const ordered = orderNodes(nodes, edges)
      const indices = nodeIds
        .map((id) => ordered.findIndex((n) => n.id === id))
        .filter((i) => i !== -1)
        .sort((a, b) => a - b)
      if (indices.length !== nodeIds.length) return false
      return indices[indices.length - 1] - indices[0] + 1 === indices.length
    },
    [nodes, edges],
  )

  const groupSelectedNodes = useCallback(() => {
    if (selectedNodeIds.length < 2) return
    if (!isConsecutiveSelection(selectedNodeIds)) {
      setStatusMessage(t("groupMustBeConsecutive"))
      return
    }
    const existingLabels = new Set(nodes.map((n) => n.data.group).filter(Boolean))
    let i = 1
    while (existingLabels.has(t("defaultGroupName", { n: String(i) }))) i++
    const label = t("defaultGroupName", { n: String(i) })
    pushHistory()
    setNodes((nds) =>
      nds.map((n) => (selectedNodeIds.includes(n.id) ? { ...n, data: { ...n.data, group: label } } : n)),
    )
    setEditingGroupLabel(label)
  }, [selectedNodeIds, isConsecutiveSelection, nodes, pushHistory, setNodes, t])

  const startEditGroup = useCallback((label: string) => setEditingGroupLabel(label), [])
  const cancelEditGroup = useCallback(() => setEditingGroupLabel(null), [])

  const commitGroupRename = useCallback(
    (oldLabel: string, newValueRaw: string) => {
      setEditingGroupLabel(null)
      const newValue = newValueRaw.trim()
      if (!newValue || newValue === oldLabel) return
      pushHistory()
      setNodes((nds) =>
        nds.map((n) => (n.data.group === oldLabel ? { ...n, data: { ...n.data, group: newValue } } : n)),
      )
    },
    [pushHistory, setNodes],
  )

  const ungroupNodes = useCallback(
    (label: string) => {
      pushHistory()
      setNodes((nds) => nds.map((n) => (n.data.group === label ? { ...n, data: { ...n.data, group: undefined } } : n)))
    },
    [pushHistory, setNodes],
  )

  const loopSelectedNodes = useCallback(() => {
    if (selectedNodeIds.length < 2) return
    if (!isConsecutiveSelection(selectedNodeIds)) {
      setStatusMessage(t("loopMustBeConsecutive"))
      return
    }
    const existingLabels = new Set(nodes.map((n) => n.data.loop).filter(Boolean))
    let i = 1
    while (existingLabels.has(t("defaultLoopName", { n: String(i) }))) i++
    const label = t("defaultLoopName", { n: String(i) })
    pushHistory()
    setNodes((nds) =>
      nds.map((n) =>
        selectedNodeIds.includes(n.id) ? { ...n, data: { ...n.data, loop: label, loopCount: 1 } } : n,
      ),
    )
    setEditingLoopLabel(label)
  }, [selectedNodeIds, isConsecutiveSelection, nodes, pushHistory, setNodes, t])

  const startEditLoop = useCallback((label: string) => setEditingLoopLabel(label), [])
  const cancelEditLoop = useCallback(() => setEditingLoopLabel(null), [])

  const commitLoopRename = useCallback(
    (oldLabel: string, newLabelRaw: string, newCount: number, newTable: string | null) => {
      setEditingLoopLabel(null)
      const newLabel = newLabelRaw.trim() || oldLabel
      const count = Number.isFinite(newCount) && newCount > 0 ? Math.floor(newCount) : 1
      const table = newTable?.trim() || undefined
      const current = nodes.find((n) => n.data.loop === oldLabel)?.data
      if (newLabel === oldLabel && current?.loopCount === count && current?.loopTable === table) return
      pushHistory()
      setNodes((nds) =>
        nds.map((n) =>
          n.data.loop === oldLabel
            ? { ...n, data: { ...n.data, loop: newLabel, loopCount: table ? undefined : count, loopTable: table } }
            : n,
        ),
      )
    },
    [pushHistory, setNodes, nodes],
  )

  const unloopNodes = useCallback(
    (label: string) => {
      pushHistory()
      setNodes((nds) =>
        nds.map((n) =>
          n.data.loop === label
            ? { ...n, data: { ...n.data, loop: undefined, loopCount: undefined, loopTable: undefined } }
            : n,
        ),
      )
    },
    [pushHistory, setNodes],
  )

  const ifSelectedNodes = useCallback(() => {
    if (selectedNodeIds.length < 2) return
    if (!isConsecutiveSelection(selectedNodeIds)) {
      setStatusMessage(t("ifMustBeConsecutive"))
      return
    }
    const orderedIds = orderNodes(nodes, edges).map((n) => n.id)
    const selectedOrdered = orderedIds.filter((id) => selectedNodeIds.includes(id))
    const firstId = selectedOrdered[0]
    const lastId = selectedOrdered[selectedOrdered.length - 1]
    const firstNode = nodes.find((n) => n.id === firstId)
    const lastNode = nodes.find((n) => n.id === lastId)
    if (!firstNode || !lastNode) return

    pushHistory()
    const ifId = String(nextNodeId++)
    const elseId = String(nextNodeId++)
    const endifId = String(nextNodeId++)
    const ifNode: Node<StepNodeData> = {
      id: ifId,
      type: "actionNode",
      position: { x: firstNode.position.x, y: firstNode.position.y - IF_NODE_GAP },
      data: { action: "if", params: {} },
    }
    const elseNode: Node<StepNodeData> = {
      id: elseId,
      type: "actionNode",
      position: { x: lastNode.position.x, y: lastNode.position.y + IF_NODE_GAP },
      data: { action: "else", params: {} },
    }
    const endifNode: Node<StepNodeData> = {
      id: endifId,
      type: "actionNode",
      position: { x: lastNode.position.x, y: lastNode.position.y + IF_NODE_GAP * 2 },
      data: { action: "endif", params: {} },
    }
    setNodes((nds) => nds.concat(ifNode, elseNode, endifNode))
    setEdges((eds) => {
      const inEdge = eds.find((e) => e.target === firstId)
      const outEdge = eds.find((e) => e.source === lastId)
      const next = eds.filter((e) => e !== inEdge && e !== outEdge)
      if (inEdge) next.push({ id: `e${inEdge.source}-${ifId}`, source: inEdge.source, target: ifId })
      next.push({ id: `e${ifId}-${firstId}`, source: ifId, target: firstId })
      next.push({ id: `e${lastId}-${elseId}`, source: lastId, target: elseId })
      next.push({ id: `e${elseId}-${endifId}`, source: elseId, target: endifId })
      if (outEdge) next.push({ id: `e${endifId}-${outEdge.target}`, source: endifId, target: outEdge.target })
      return next
    })
    setSelectedNodeId(ifId)
  }, [selectedNodeIds, isConsecutiveSelection, nodes, edges, pushHistory, setNodes, setEdges, t])

  const addElseBranch = useCallback(
    (ifId: string) => {
      const frame = computeIfFrames(nodes, edges).find((f) => f.ifId === ifId)
      if (!frame || frame.elseId) return
      const endifNode = nodes.find((n) => n.id === frame.endifId)
      if (!endifNode) return

      pushHistory()
      const elseId = String(nextNodeId++)
      const elseNode: Node<StepNodeData> = {
        id: elseId,
        type: "actionNode",
        position: { x: endifNode.position.x, y: endifNode.position.y - IF_NODE_GAP },
        data: { action: "else", params: {} },
      }
      setNodes((nds) => nds.concat(elseNode))
      setEdges((eds) => {
        const inEdge = eds.find((e) => e.target === frame.endifId)
        const next = eds.filter((e) => e !== inEdge)
        if (inEdge) next.push({ id: `e${inEdge.source}-${elseId}`, source: inEdge.source, target: elseId })
        next.push({ id: `e${elseId}-${frame.endifId}`, source: elseId, target: frame.endifId })
        return next
      })
    },
    [nodes, edges, pushHistory, setNodes, setEdges],
  )

  const removeIfBlock = useCallback(
    (ifId: string) => {
      const frame = computeIfFrames(nodes, edges).find((f) => f.ifId === ifId)
      if (!frame) return
      const idsToRemove = [frame.ifId, frame.elseId, frame.endifId].filter((id): id is string => Boolean(id))

      pushHistory()
      setNodes((nds) => nds.filter((n) => !idsToRemove.includes(n.id)))
      setEdges((eds) => {
        let result = eds
        for (const id of idsToRemove) {
          const inEdge = result.find((e) => e.target === id)
          const outEdge = result.find((e) => e.source === id)
          result = result.filter((e) => e.source !== id && e.target !== id)
          if (inEdge && outEdge) {
            result = result.concat({ id: `e${inEdge.source}-${outEdge.target}`, source: inEdge.source, target: outEdge.target })
          }
        }
        return result
      })
      setSelectedNodeId(null)
    },
    [nodes, edges, pushHistory, setNodes, setEdges],
  )

  const applyAiSuggestion = useCallback(
    (nodeId: string, action: string, params: Record<string, unknown>) => {
      pushHistory()
      setNodes((nds) =>
        nds.map((n) =>
          n.id === nodeId ? { ...n, type: nodeTypeForAction(action), data: { ...n.data, action, params } } : n,
        ),
      )
    },
    [pushHistory, setNodes],
  )

  const insertAiActionsAfterSelected = useCallback(
    (actions: FlowAction[]) => {
      if (!selectedNodeId || actions.length === 0) return
      const selected = nodes.find((node) => node.id === selectedNodeId)
      if (!selected || selected.data.action === "end") return

      const outgoing = edges.filter((edge) => edge.source === selectedNodeId)
      if (outgoing.length > 1) {
        setStatusMessage(t("chatActionsInsertUnavailable"))
        return
      }

      pushHistory()
      const newNodes: Node<StepNodeData>[] = actions.map((item, index) => ({
        id: String(nextNodeId++),
        type: nodeTypeForAction(item.action),
        position: {
          x: selected.position.x,
          y: selected.position.y + (index + 1) * (NODE_BOX_HEIGHT + 74),
        },
        data: { action: item.action, params: item.params },
      }))
      const target = outgoing[0]?.target
      const chain = [selectedNodeId, ...newNodes.map((node) => node.id)]

      const shiftY = newNodes.length * (NODE_BOX_HEIGHT + 74)
      setNodes((nds) =>
        target ? shiftReachableNodesDown(nds.concat(newNodes), edges, target, shiftY) : nds.concat(newNodes),
      )
      setEdges((eds) => {
        const next = eds.filter((edge) => edge.source !== selectedNodeId)
        const insertedEdges = chain.slice(0, -1).map((source, index) => ({
          id: "e" + source + "-" + chain[index + 1],
          source,
          target: chain[index + 1],
        }))
        if (target) {
          insertedEdges.push({
            id: "e" + newNodes[newNodes.length - 1].id + "-" + target,
            source: newNodes[newNodes.length - 1].id,
            target,
          })
        }
        return next.concat(insertedEdges)
      })
    },
    [edges, nodes, pushHistory, selectedNodeId, setEdges, setNodes, t],
  )

  const handleSave = useCallback(async () => {
    if (!currentFile) return
    const steps = flowToSteps(nodes, edges)
    try {
      setStatusMessage(t("savingScenario"))
      await saveScenario(currentFile, title, steps)
      setSavedSnapshot(JSON.stringify({ title, steps }))
      setScenarioSummaries((summaries) =>
        summaries.map((s) => (s.filename === currentFile ? { ...s, title } : s)),
      )
      setStatusMessage(t("saveScenarioSuccess", { file: currentFile }))
    } catch {
      setStatusMessage(t("saveScenarioFailed"))
    }
  }, [currentFile, title, nodes, edges, t])

  const handleSaveAs = useCallback(async () => {
    const defaultName = currentFile.split("/").pop() ?? ""
    const steps = flowToSteps(nodes, edges)
    try {
      setStatusMessage(t("savingScenarioAs"))
      const filename = await pickScenarioSaveFile(defaultName)
      if (!filename) return
      await saveScenario(filename, title, steps)
      setSavedSnapshot(JSON.stringify({ title, steps }))
      await refreshScenarioList()
      setCurrentFile(filename)
      setStatusMessage(t("saveScenarioSuccess", { file: filename }))
    } catch (e) {
      const detail = axios.isAxiosError(e) ? (e.response?.data as { detail?: unknown } | undefined)?.detail : undefined
      setStatusMessage(typeof detail === "string" ? detail : t("saveAsFailed"))
    }
  }, [currentFile, title, nodes, edges, refreshScenarioList, t])

  const handleNewScenario = useCallback(async () => {
    const raw = window.prompt(t("newScenarioPlaceholder"))
    if (!raw) return
    const filename = /\.ya?ml$/i.test(raw) ? raw : `${raw}.yaml`
    try {
      await createScenario(filename, "", [{ action: "start" }, { action: "end" }])
      await refreshScenarioList()
      setCurrentFile(filename)
      setStatusMessage(t("saveScenarioSuccess", { file: filename }))
    } catch {
      setStatusMessage(t("createScenarioFailed"))
    }
  }, [refreshScenarioList, t])

  useScenarioShortcuts({
    lastMousePositionRef,
    handleSave,
    toggleFocusMode,
    undo,
    redo,
    copySelectedNodes,
    cutSelectedNodes,
    pasteNodesAt,
    screenToFlowPosition,
  })

  // Promote the toolbar's transient status label to a permanent bottom status bar, so it
  // must self-clear rather than sit stale until the next message.
  useEffect(() => {
    if (!statusMessage) return
    const timer = window.setTimeout(() => setStatusMessage(""), 4000)
    return () => window.clearTimeout(timer)
  }, [statusMessage])

  const { handleRun, handleStop } = useScenarioExecution({
    currentFile,
    running,
    nodes,
    edges,
    t,
    setLogs,
    setRunning,
    setRunningStep,
    setStatusMessage,
    setRunContext,
  })

  const selectedNode =
    nodes.find((n) => n.id === selectedNodeId && n.data.action !== "load_table") ?? null
  const selectedNodeForPanel = useMemo(() => {
    if (!selectedNode) return null
    const flowStep = orderNodes(nodes, edges).findIndex((node) => node.id === selectedNode.id) + 1
    return { ...selectedNode, data: { ...selectedNode.data, flowStep } }
  }, [edges, nodes, selectedNode])
  const currentSnapshot = useMemo(() => JSON.stringify({ title, steps: flowToSteps(nodes, edges) }), [title, nodes, edges])
  const hasUnsavedChanges = Boolean(currentFile && savedSnapshot !== null && currentSnapshot !== savedSnapshot)

  const { renderNodes, renderEdges } = useMemo(() => {
    const groupLabels = Array.from(new Set(nodes.map((n) => n.data.group).filter((g): g is string => Boolean(g))))
    const groupFrameNodes: Node[] = groupLabels.map((label) => {
      const members = nodes.filter((n) => n.data.group === label)
      const minX = Math.min(...members.map((n) => n.position.x))
      const minY = Math.min(...members.map((n) => n.position.y))
      const maxX = Math.max(...members.map((n) => n.position.x + NODE_BOX_WIDTH))
      const maxY = Math.max(...members.map((n) => n.position.y + NODE_BOX_HEIGHT))
      const frameWidth = maxX - minX + GROUP_PADDING * 2
      const frameHeight = maxY - minY + GROUP_PADDING * 2 + GROUP_LABEL_SPACE
      return {
        id: `group-${label}`,
        type: "groupFrameNode",
        position: { x: minX - GROUP_PADDING, y: minY - GROUP_PADDING - GROUP_LABEL_SPACE },
        width: frameWidth,
        height: frameHeight,
        style: { width: frameWidth, height: frameHeight },
        draggable: false,
        selectable: false,
        connectable: false,
        focusable: false,
        zIndex: -1,
        data: {
          label,
          isEditing: label === editingGroupLabel,
          onStartEdit: startEditGroup,
          onCommitEdit: commitGroupRename,
          onCancelEdit: cancelEditGroup,
        },
      }
    })

    const loopLabels = Array.from(new Set(nodes.map((n) => n.data.loop).filter((l): l is string => Boolean(l))))
    const tableNames = listTables(nodes, edges)
    const loopFrameNodes: Node[] = loopLabels.map((label) => {
      const members = nodes.filter((n) => n.data.loop === label)
      const minX = Math.min(...members.map((n) => n.position.x))
      const minY = Math.min(...members.map((n) => n.position.y))
      const maxX = Math.max(...members.map((n) => n.position.x + NODE_BOX_WIDTH))
      const maxY = Math.max(...members.map((n) => n.position.y + NODE_BOX_HEIGHT))
      const frameWidth = maxX - minX + GROUP_PADDING * 2
      const frameHeight = maxY - minY + GROUP_PADDING * 2 + GROUP_LABEL_SPACE
      return {
        id: `loop-${label}`,
        type: "loopFrameNode",
        position: { x: minX - GROUP_PADDING, y: minY - GROUP_PADDING - GROUP_LABEL_SPACE },
        width: frameWidth,
        height: frameHeight,
        style: { width: frameWidth, height: frameHeight },
        draggable: false,
        selectable: false,
        connectable: false,
        focusable: false,
        zIndex: -1,
        data: {
          label,
          count: members[0]?.data.loopCount ?? 1,
          loopTable: members[0]?.data.loopTable,
          tableNames,
          isEditing: label === editingLoopLabel,
          onStartEdit: startEditLoop,
          onCommitEdit: commitLoopRename,
          onCancelEdit: cancelEditLoop,
        },
      }
    })

    const ordered = orderNodes(nodes, edges)
    const orderedIndex = new Map(ordered.map((n, i) => [n.id, i]))
    const nodeById = new Map(nodes.map((n) => [n.id, n]))
    const ifFrameNodes: Node[] = computeIfFrames(nodes, edges).flatMap((frame) => {
      const startIdx = orderedIndex.get(frame.ifId)
      const endIdx = orderedIndex.get(frame.endifId)
      if (startIdx == null || endIdx == null) return []
      const members = ordered.slice(Math.min(startIdx, endIdx), Math.max(startIdx, endIdx) + 1)
      const minX = Math.min(...members.map((n) => n.position.x))
      const minY = Math.min(...members.map((n) => n.position.y))
      const maxX = Math.max(...members.map((n) => n.position.x + NODE_BOX_WIDTH))
      const maxY = Math.max(...members.map((n) => n.position.y + NODE_BOX_HEIGHT))
      const frameWidth = maxX - minX + GROUP_PADDING * 2
      const frameHeight = maxY - minY + GROUP_PADDING * 2 + GROUP_LABEL_SPACE
      const ifNode = nodeById.get(frame.ifId)
      const elseNode = frame.elseId ? nodeById.get(frame.elseId) : undefined
      const dividerTop = elseNode
        ? elseNode.position.y - minY + GROUP_PADDING + GROUP_LABEL_SPACE - IF_NODE_GAP / 2
        : undefined
      return [
        {
          id: `if-${frame.ifId}`,
          type: "ifFrameNode",
          position: { x: minX - GROUP_PADDING, y: minY - GROUP_PADDING - GROUP_LABEL_SPACE },
          width: frameWidth,
          height: frameHeight,
          style: { width: frameWidth, height: frameHeight },
          draggable: false,
          selectable: false,
          connectable: false,
          focusable: false,
          zIndex: -1,
          data: {
            ifId: frame.ifId,
            variable: typeof ifNode?.data.params.variable === "string" ? ifNode.data.params.variable : "",
            equals: typeof ifNode?.data.params.equals === "string" ? ifNode.data.params.equals : undefined,
            last_step: typeof ifNode?.data.params.last_step === "string" ? ifNode.data.params.last_step : undefined,
            dividerTop,
          },
        },
      ]
    })

    const hiddenNodeIds = new Set(nodes.filter((node) => node.data.action === "load_table").map((node) => node.id))
    const visibleEdges = edges.filter((edge) => !hiddenNodeIds.has(edge.source) && !hiddenNodeIds.has(edge.target))
    // React Flow's default edge is Bezier, so aligned nodes can still appear connected
    // by a curve. Use the built-in straight edge for ordinary flow connections.
    const renderEdges = visibleEdges.map((edge) => {
      const highlighted = edge.id === dragOverEdgeId
      return {
        ...edge,
        type: "straight",
        markerEnd: {
          type: MarkerType.ArrowClosed,
          width: 20,
          height: 20,
          color: highlighted ? "#aa3bff" : "#7c3aed",
        },
        ...(highlighted
          ? { style: { ...edge.style, stroke: "#aa3bff", strokeWidth: 3 }, animated: true }
          : {}),
      }
    })

    return {
      renderNodes: [
        ...groupFrameNodes,
        ...loopFrameNodes,
        ...ifFrameNodes,
        ...nodes
          .filter((node) => node.data.action !== "load_table")
          .map((node) => ({
            ...node,
            // The custom node renders the accessible control. Keeping React Flow's
            // wrapper unfocusable avoids two nested buttons for every node.
            focusable: false,
            data: {
              ...node.data,
              flowStep: (orderedIndex.get(node.id) ?? 0) + 1,
              onRequestContextMenu: (x: number, y: number) => openNodeContextMenu(x, y, node),
              onNudgeNode: nudgeNode,
              onFocusAdjacentNode: focusAdjacentNode,
            },
          })),
      ],
      renderEdges,
    }
  }, [
    nodes,
    edges,
    editingGroupLabel,
    startEditGroup,
    commitGroupRename,
    cancelEditGroup,
    editingLoopLabel,
    startEditLoop,
    commitLoopRename,
    cancelEditLoop,
    dragOverEdgeId,
    openNodeContextMenu,
    nudgeNode,
    focusAdjacentNode,
  ])

  const stepIndexOf = useCallback(
    (nodeId: string) => orderNodes(nodes, edges).findIndex((n) => n.id === nodeId) + 1,
    [nodes, edges],
  )

  const rightClickedNodeGroup = contextMenu?.nodeId
    ? nodes.find((n) => n.id === contextMenu.nodeId)?.data.group
    : undefined

  const rightClickedNodeLoop = contextMenu?.nodeId
    ? nodes.find((n) => n.id === contextMenu.nodeId)?.data.loop
    : undefined

  const rightClickedNodeAction = contextMenu?.nodeId
    ? nodes.find((n) => n.id === contextMenu.nodeId)?.data.action
    : undefined

  const rightClickedIfFrame = contextMenu?.ifId
    ? computeIfFrames(nodes, edges).find((f) => f.ifId === contextMenu.ifId)
    : undefined

  const rerunStepWithCountdown = useCallback((start: number, end = start) => {
    if (pendingRunTimeoutRef.current !== null) window.clearTimeout(pendingRunTimeoutRef.current)
    const scheduledFile = currentFileRef.current
    setStatusMessage(t("runThisStepOnlyCountdown"))
    pendingRunTimeoutRef.current = window.setTimeout(() => {
      pendingRunTimeoutRef.current = null
      if (currentFileRef.current !== scheduledFile || runningRef.current) {
        setStatusMessage(t("runThisStepOnlyCancelled"))
        return
      }
      recordRecoveryStarted()
      handleRun({ start, end })
    }, 3000)
  }, [handleRun, setStatusMessage, t])

  const contextMenuSections = contextMenu
    ? contextMenu.nodeId
      ? [
          {
            title: t("debugRun"),
            items: [
              {
                label: t("runFromHere"),
                onClick: () => handleRun({ start: stepIndexOf(contextMenu.nodeId as string) }),
              },
              {
                label: t("runToHere"),
                onClick: () => handleRun({ end: stepIndexOf(contextMenu.nodeId as string) }),
              },
              {
                label: t("runThisStepOnly"),
                onClick: () => {
                  // If the right-clicked node is part of a multi-selection, run the whole
                  // selected span (min to max step index) rather than just that one node —
                  // the runner can only execute a contiguous range, so a non-contiguous
                  // selection also runs whatever's in between.
                  const inSelection =
                    selectedNodeIds.length >= 2 && selectedNodeIds.includes(contextMenu.nodeId as string)
                  const steps = (inSelection ? selectedNodeIds : [contextMenu.nodeId as string]).map(stepIndexOf)
                  const start = Math.min(...steps)
                  const end = Math.max(...steps)
                  // A 3s delay before firing, so there's time to switch to the target window
                  // first — running instantly makes this hard to actually try out. Cancel any
                  // previous pending countdown, and bail out at fire time (rather than running
                  // against stale step indices, or stacking a second concurrent run) if the
                  // active scenario changed or another run started in the meantime.
                  rerunStepWithCountdown(start, end)
                },
              },
            ],
          },
          ...(selectedNodeIds.length >= 2 && selectedNodeIds.includes(contextMenu.nodeId)
            ? [
                {
                  items: [
                    { label: t("groupSelected"), onClick: groupSelectedNodes },
                    { label: t("loopSelected"), onClick: loopSelectedNodes },
                    { label: t("ifSelected"), onClick: ifSelectedNodes },
                  ],
                },
              ]
            : []),
          ...(rightClickedNodeGroup
            ? [
                {
                  items: [
                    { label: t("renameGroup"), onClick: () => startEditGroup(rightClickedNodeGroup as string) },
                    {
                      label: t("ungroup"),
                      onClick: () => ungroupNodes(rightClickedNodeGroup as string),
                      danger: true,
                    },
                  ],
                },
              ]
            : []),
          ...(rightClickedNodeLoop
            ? [
                {
                  items: [
                    { label: t("editLoop"), onClick: () => startEditLoop(rightClickedNodeLoop as string) },
                    {
                      label: t("removeLoop"),
                      onClick: () => unloopNodes(rightClickedNodeLoop as string),
                      danger: true,
                    },
                  ],
                },
              ]
            : []),
          ...(rightClickedNodeAction && rightClickedNodeAction !== "start" && rightClickedNodeAction !== "end"
            ? [
                {
                  items: [{ label: t("aiSuggest"), onClick: () => setAiSuggestNodeId(contextMenu.nodeId as string) }],
                },
              ]
            : []),
          {
            items: [
              { label: t("copy"), onClick: () => copyNode(contextMenu.nodeId as string) },
              { label: t("delete"), onClick: () => deleteNode(contextMenu.nodeId as string), danger: true },
            ],
          },
        ]
      : contextMenu.edgeId
        ? [
            {
              items: [
                { label: t("deleteEdge"), onClick: () => deleteEdge(contextMenu.edgeId as string), danger: true },
              ],
            },
          ]
        : contextMenu.groupLabel
          ? [
              {
                items: [
                  { label: t("renameGroup"), onClick: () => startEditGroup(contextMenu.groupLabel as string) },
                  {
                    label: t("ungroup"),
                    onClick: () => ungroupNodes(contextMenu.groupLabel as string),
                    danger: true,
                  },
                ],
              },
            ]
          : contextMenu.loopLabel
            ? [
                {
                  items: [
                    { label: t("editLoop"), onClick: () => startEditLoop(contextMenu.loopLabel as string) },
                    {
                      label: t("removeLoop"),
                      onClick: () => unloopNodes(contextMenu.loopLabel as string),
                      danger: true,
                    },
                  ],
                },
              ]
            : contextMenu.ifId
              ? [
                  {
                    items: [
                      ...(!rightClickedIfFrame?.elseId
                        ? [{ label: t("addElseBranch"), onClick: () => addElseBranch(contextMenu.ifId as string) }]
                        : []),
                      {
                        label: t("removeIf"),
                        onClick: () => removeIfBlock(contextMenu.ifId as string),
                        danger: true,
                      },
                    ],
                  },
                ]
              : [
              ...(clipboardRef.current
                ? [
                    {
                      items: [
                        {
                          label: t("paste"),
                          onClick: () => {
                            const position = screenToFlowPosition(contextMenu.x, contextMenu.y)
                            if (position) pasteNodesAt(position)
                          },
                        },
                      ],
                    },
                  ]
                : []),
              {
                title: t("addAction"),
                items: schemas.map((schema) => ({
                  label: schema.labels[locale],
                  onClick: () => {
                    const position = screenToFlowPosition(contextMenu.x, contextMenu.y)
                    if (position) addNode(schema.action, position)
                  },
                })),
              },
            ]
    : []

  const runningNodeId = useMemo(() => {
    if (runningStep == null) return null
    return orderNodes(nodes, edges)[runningStep - 1]?.id ?? null
  }, [runningStep, nodes, edges])

  const jumpToStep = useCallback(
    (step: number) => {
      const node = orderNodes(nodes, edges)[step - 1]
      if (!node) return
      setSelectedNodeId(node.id)
      setSelectedNodeIds([node.id])
      reactFlowInstance?.fitView({ nodes: [node], duration: 300, padding: 0.3 })
    },
    [nodes, edges, reactFlowInstance],
  )

  const variableList = useMemo(() => listVariables(nodes, edges), [nodes, edges])

  return (
    <div className="editor-layout">
      <a className="skip-link" href="#scenario-editor-canvas">{t("skipToCanvas")}</a>
      <div className="app-header">
        <TitleBar />
        <MenuBar
          onSelectScenario={setCurrentFile}
          onNewScenario={() => void handleNewScenario()}
          onSave={() => void handleSave()}
          saveDisabled={!currentFile}
          onSaveAs={() => void handleSaveAs()}
          saveAsDisabled={!currentFile}
          onRun={() => void handleRun()}
          onStop={() => void handleStop()}
          running={running}
          runDisabled={!currentFile}
          onUndo={undo}
          undoDisabled={past.length === 0}
          onRedo={redo}
          redoDisabled={future.length === 0}
          version={version}
          focusMode={focusMode}
          onToggleFocusMode={toggleFocusMode}
        />
        <span className="scenario-title-display" title={currentFile || t("scenarioTitlePlaceholder")}>
          {currentFile.replace(/\.ya?ml$/i, "") || t("scenarioTitlePlaceholder")}
          {hasUnsavedChanges && (
            <span className="unsaved-indicator" title={t("unsavedChanges")} aria-label={t("unsavedChanges")}>
              ●
            </span>
          )}
        </span>
      </div>
      <FirstUseGuide />
      <div
        className={`editor-body${focusMode ? " focus-mode" : ""}`}
        style={
          {
            "--palette-width": `${paletteWidth}px`,
            "--parameter-panel-width": `${parameterPanelWidth}px`,
          } as CSSProperties
        }
      >
        <ActionLabelProvider value={actionLabels}>
          <RunningNodeProvider value={runningNodeId}>
            <ActionPalette
              schemas={schemas.filter((schema) => schema.action !== "load_table")}
              onAddAction={addActionFromPalette}
            />
            <PanelResizeHandle
              side="palette"
              width={paletteWidth}
              minWidth={PALETTE_WIDTH.min}
              maxWidth={PALETTE_WIDTH.max}
              onWidthChange={setPaletteWidth}
            />
            <FlowCanvas
              nodes={nodes}
              renderNodes={renderNodes}
              renderEdges={renderEdges}
              schemas={schemas}
              currentFile={currentFile}
              running={running}
              selectedNode={selectedNode}
              aiSuggestNodeId={aiSuggestNodeId}
              showTableImport={showTableImport}
              contextMenu={contextMenu}
              contextMenuSections={contextMenuSections}
              reactFlowWrapper={reactFlowWrapper}
              onNodesChange={handleNodesChange}
              onEdgesChange={handleEdgesChange}
              onConnect={onConnect}
              onNodeClick={onNodeClick}
              onNodeDragStart={onNodeDragStart}
              onNodeDragStop={onNodeDragStop}
              onSelectionChange={onSelectionChange}
              onPaneClick={onPaneClick}
              onNodeContextMenu={onNodeContextMenu}
              onEdgeContextMenu={onEdgeContextMenu}
              onPaneContextMenu={onPaneContextMenu}
              onDrop={onDrop}
              onDragOver={onDragOver}
              onDragLeave={onDragLeave}
              onMouseMove={handleCanvasMouseMove}
              onInit={setReactFlowInstance}
              onRun={() => void handleRun()}
              onStop={() => void handleStop()}
              onAlign={handleAlign}
              onCloseContextMenu={closeContextMenu}
              onInsertAiActions={insertAiActionsAfterSelected}
              onApplyAiSuggestion={applyAiSuggestion}
              onCloseAiSuggestion={() => setAiSuggestNodeId(null)}
              onImportTable={handleTableImport}
              onCloseTableImport={() => setShowTableImport(false)}
              existingTableNames={listTables(nodes, edges)}
            />
            <PanelResizeHandle
              side="parameter"
              width={parameterPanelWidth}
              minWidth={PARAMETER_PANEL_WIDTH.min}
              maxWidth={PARAMETER_PANEL_WIDTH.max}
              onWidthChange={setParameterPanelWidth}
            />
            <ParameterPanel
              node={selectedNodeForPanel}
              schemas={schemas}
              scenarioFilename={currentFile}
              scenarioFilenames={scenarioSummaries.map((s) => s.filename)}
              variableNames={variableList.map((v) => v.name)}
              onChange={updateNodeParams}
              onCommit={commitParamEdit}
              onDelete={deleteNode}
            />
          </RunningNodeProvider>
        </ActionLabelProvider>
      </div>
      <ExecutionPanel
        logs={logs}
        running={running}
        variables={variableList}
        importedTable={importedTable}
        onImportTable={() => setShowTableImport(true)}
        onImportedRowsChange={handleImportedRowsChange}
        onStepClick={jumpToStep}
        onRerunStep={rerunStepWithCountdown}
        runContext={runContext}
      />
      <StatusBar currentFile={currentFile} message={statusMessage} version={version} unsaved={hasUnsavedChanges} />
    </div>
  )
}
