import { lazy, Suspense, type ComponentProps, type MouseEvent as ReactMouseEvent, type RefObject } from "react"
import ReactFlow, {
  Background,
  ControlButton,
  Controls,
  MiniMap,
  Panel,
  type Edge,
  type Node,
  type NodeChange,
  type ReactFlowInstance,
} from "reactflow"
import "reactflow/dist/style.css"
import type { ActionSchema, StepNodeData } from "../types/scenario"
import type { TableImportResult } from "./TableImportModal"
import ActionNode from "./ActionNode"
import ChatWidget from "./ChatWidget"
import ContextMenu, { type ContextMenuSection } from "./ContextMenu"
import GroupFrameNode from "./GroupFrameNode"
import { AlignIcon, PlayIcon, StopIcon } from "./icons"
import IfFrameNode from "./IfFrameNode"
import LoopFrameNode from "./LoopFrameNode"
import TableImportModal from "./TableImportModal"
import TerminalNode from "./TerminalNode"
import { useLocale } from "../i18n/useLocale"

const AiSuggestModal = lazy(() => import("./AiSuggestModal"))

const nodeTypes = {
  actionNode: ActionNode,
  terminalNode: TerminalNode,
  groupFrameNode: GroupFrameNode,
  loopFrameNode: LoopFrameNode,
  ifFrameNode: IfFrameNode,
}
const edgeTypes = {}

interface Props {
  nodes: Node<StepNodeData>[]
  renderNodes: Node[]
  renderEdges: Edge[]
  schemas: ActionSchema[]
  currentFile: string
  running: boolean
  selectedNode: Node<StepNodeData> | null
  aiSuggestNodeId: string | null
  showTableImport: boolean
  contextMenu: { x: number; y: number } | null
  contextMenuSections: ContextMenuSection[]
  reactFlowWrapper: RefObject<HTMLDivElement | null>
  onNodesChange: (changes: NodeChange[]) => void
  onEdgesChange: NonNullable<ComponentProps<typeof ReactFlow>["onEdgesChange"]>
  onConnect: NonNullable<ComponentProps<typeof ReactFlow>["onConnect"]>
  onNodeClick: NonNullable<ComponentProps<typeof ReactFlow>["onNodeClick"]>
  onNodeDragStart: NonNullable<ComponentProps<typeof ReactFlow>["onNodeDragStart"]>
  onNodeDragStop: NonNullable<ComponentProps<typeof ReactFlow>["onNodeDragStop"]>
  onSelectionChange: NonNullable<ComponentProps<typeof ReactFlow>["onSelectionChange"]>
  onPaneClick: () => void
  onNodeContextMenu: NonNullable<ComponentProps<typeof ReactFlow>["onNodeContextMenu"]>
  onEdgeContextMenu: NonNullable<ComponentProps<typeof ReactFlow>["onEdgeContextMenu"]>
  onPaneContextMenu: NonNullable<ComponentProps<typeof ReactFlow>["onPaneContextMenu"]>
  onDrop: NonNullable<ComponentProps<typeof ReactFlow>["onDrop"]>
  onDragOver: NonNullable<ComponentProps<typeof ReactFlow>["onDragOver"]>
  onDragLeave: () => void
  onMouseMove: (event: ReactMouseEvent<HTMLDivElement>) => void
  onInit: (instance: ReactFlowInstance) => void
  onRun: () => void
  onStop: () => void
  onAlign: () => void
  onCloseContextMenu: () => void
  onInsertAiActions: (actions: Array<{ action: string; params: Record<string, unknown> }>) => void
  onApplyAiSuggestion: (nodeId: string, action: string, params: Record<string, unknown>) => void
  onCloseAiSuggestion: () => void
  onImportTable: (result: TableImportResult) => void
  onCloseTableImport: () => void
  existingTableNames: string[]
}

export default function FlowCanvas({
  nodes,
  renderNodes,
  renderEdges,
  schemas,
  currentFile,
  running,
  selectedNode,
  aiSuggestNodeId,
  showTableImport,
  contextMenu,
  contextMenuSections,
  reactFlowWrapper,
  onNodesChange,
  onEdgesChange,
  onConnect,
  onNodeClick,
  onNodeDragStart,
  onNodeDragStop,
  onSelectionChange,
  onPaneClick,
  onNodeContextMenu,
  onEdgeContextMenu,
  onPaneContextMenu,
  onDrop,
  onDragOver,
  onDragLeave,
  onMouseMove,
  onInit,
  onRun,
  onStop,
  onAlign,
  onCloseContextMenu,
  onInsertAiActions,
  onApplyAiSuggestion,
  onCloseAiSuggestion,
  onImportTable,
  onCloseTableImport,
  existingTableNames,
}: Props) {
  const { t } = useLocale()

  return (
    <div
      className="flow-canvas"
      id="scenario-editor-canvas"
      role="region"
      tabIndex={-1}
      aria-label={t("flowCanvas")}
      ref={reactFlowWrapper}
      onMouseMove={onMouseMove}
    >
      <p id="flow-keyboard-help" className="visually-hidden">{t("flowKeyboardHelp")}</p>
      <ReactFlow
        nodes={renderNodes}
        edges={renderEdges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
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
        onInit={onInit}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        deleteKeyCode={["Backspace", "Delete"]}
        fitView
      >
        <Background />
        <Panel position="top-right" className="flow-legend" aria-label={t("flowLegend")}>
          <span className="flow-legend-title">{t("flowLegend")}</span>
          <span className="flow-legend-item"><span className="flow-legend-swatch" />{t("flowLegendAction")}</span>
          <span className="flow-legend-item"><span className="flow-legend-swatch running" />{t("flowLegendRunning")}</span>
          <span className="flow-legend-item"><span className="flow-legend-swatch branch" />{t("flowLegendBranch")}</span>
        </Panel>
        <Panel position="top-left" className="flow-run-controls nopan">
          <button className="flow-run-controls-button flow-run-controls-run" aria-label={t("run")} onClick={onRun} disabled={running || !currentFile} title={t("run")}>
            <PlayIcon size={16} />
          </button>
          <button className="flow-run-controls-button flow-run-controls-stop" aria-label={t("stop")} onClick={onStop} disabled={!running} title={t("stop")}>
            <StopIcon size={16} />
          </button>
        </Panel>
        <Controls>
          <ControlButton className="align-control-button" onClick={onAlign} title={t("alignNodes")} aria-label={t("alignNodes")}>
            <AlignIcon size={14} />
          </ControlButton>
        </Controls>
        <MiniMap />
      </ReactFlow>
      {contextMenu && <ContextMenu x={contextMenu.x} y={contextMenu.y} sections={contextMenuSections} onClose={onCloseContextMenu} />}
      {nodes.length === 0 && (
        <div className="canvas-empty-state">
          <strong>{t("canvasEmptyTitle")}</strong>
          <p>{t("canvasEmptyDescription")}</p>
          <p>{t("canvasEmptyStep")}</p>
        </div>
      )}
      <ChatWidget
        selectedStep={selectedNode ? { action: selectedNode.data.action, params: selectedNode.data.params } : null}
        onInsertActions={onInsertAiActions}
      />
      {aiSuggestNodeId && (() => {
        const node = nodes.find((candidate) => candidate.id === aiSuggestNodeId)
        if (!node) return null
        return (
          <Suspense fallback={null}>
            <AiSuggestModal
              action={node.data.action}
              params={node.data.params}
              schemas={schemas}
              onApply={(action, params) => onApplyAiSuggestion(aiSuggestNodeId, action, params)}
              onClose={onCloseAiSuggestion}
            />
          </Suspense>
        )
      })()}
      {showTableImport && (
        <Suspense fallback={null}>
          <TableImportModal existingNames={existingTableNames} onImport={onImportTable} onClose={onCloseTableImport} />
        </Suspense>
      )}
    </div>
  )
}
