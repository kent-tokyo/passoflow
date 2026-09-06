import type { Edge, Node } from "reactflow"
import type { ScenarioStep, StepNodeData } from "../types/scenario"

const X_POS = 250
// Must leave enough of a gap above a node for a group frame's label if that node is the
// first member of a group: the frame extends GROUP_PADDING(28) + GROUP_LABEL_SPACE(24) above
// its first member, and the label itself sits 11px above the frame's own top edge (see
// GroupFrameNode's CSS) — 63px of clearance needed above NODE_BOX_HEIGHT(66) in
// ScenarioEditor.tsx, or the previous node in the column visually covers the group label.
const Y_STEP = 140

/** start/end are dedicated actions rendered as the pill-shaped terminal node instead of a regular box. */
export function nodeTypeForAction(action: string): "terminalNode" | "actionNode" {
  return action === "start" || action === "end" ? "terminalNode" : "actionNode"
}

/** Lay scenario steps out as a straight vertical chain of nodes. */
export function stepsToFlow(steps: ScenarioStep[]): { nodes: Node<StepNodeData>[]; edges: Edge[] } {
  const nodes: Node<StepNodeData>[] = steps.map((step, i) => {
    const { action, group, loop, loop_count, loop_table, ...params } = step
    return {
      id: String(i + 1),
      type: nodeTypeForAction(action),
      position: { x: X_POS, y: i * Y_STEP },
      data: {
        action,
        params,
        group: typeof group === "string" ? group : undefined,
        loop: typeof loop === "string" ? loop : undefined,
        loopCount: typeof loop_count === "number" ? loop_count : undefined,
        loopTable: typeof loop_table === "string" ? loop_table : undefined,
      },
    }
  })

  const edges: Edge[] = []
  for (let i = 0; i < nodes.length - 1; i++) {
    edges.push({ id: `e${nodes[i].id}-${nodes[i + 1].id}`, source: nodes[i].id, target: nodes[i + 1].id })
  }

  return { nodes, edges }
}

/** Walk the edge chain from the node with no incoming edge to derive step order. */
export function orderNodes(nodes: Node<StepNodeData>[], edges: Edge[]): Node<StepNodeData>[] {
  if (nodes.length === 0) return []

  const outgoing = new Map<string, string>()
  const hasIncoming = new Set<string>()
  for (const edge of edges) {
    outgoing.set(edge.source, edge.target)
    hasIncoming.add(edge.target)
  }

  const nodeById = new Map(nodes.map((n) => [n.id, n]))
  const startNode = nodes.find((n) => !hasIncoming.has(n.id)) ?? nodes[0]

  const ordered: Node<StepNodeData>[] = []
  const visited = new Set<string>()
  let current: string | undefined = startNode.id
  while (current && !visited.has(current)) {
    visited.add(current)
    const node = nodeById.get(current)
    if (!node) break
    ordered.push(node)
    current = outgoing.get(current)
  }

  // Disconnected nodes (not reached by the chain walk) are appended in their original order.
  for (const node of nodes) {
    if (!visited.has(node.id)) ordered.push(node)
  }

  return ordered
}

/** Reposition nodes into the same straight vertical chain layout used when a scenario is first loaded. */
export function alignNodesVertically(nodes: Node<StepNodeData>[], edges: Edge[]): Node<StepNodeData>[] {
  const ordered = orderNodes(nodes, edges)
  const positionById = new Map(ordered.map((node, i) => [node.id, { x: X_POS, y: i * Y_STEP }]))
  return nodes.map((node) => (positionById.has(node.id) ? { ...node, position: positionById.get(node.id)! } : node))
}

/** Move the node at startId and every node reachable after it down by deltaY.
 * This preserves each node's horizontal/branch layout while making room for inserted steps. */
export function shiftReachableNodesDown(
  nodes: Node<StepNodeData>[],
  edges: Edge[],
  startId: string,
  deltaY: number,
): Node<StepNodeData>[] {
  const nextBySource = new Map<string, string[]>()
  for (const edge of edges) {
    const targets = nextBySource.get(edge.source) ?? []
    targets.push(edge.target)
    nextBySource.set(edge.source, targets)
  }

  const affected = new Set<string>([startId])
  const queue = [startId]
  for (let index = 0; index < queue.length; index++) {
    const current = queue[index]
    for (const target of nextBySource.get(current) ?? []) {
      if (!affected.has(target)) {
        affected.add(target)
        queue.push(target)
      }
    }
  }

  return nodes.map((node) =>
    affected.has(node.id) ? { ...node, position: { ...node.position, y: node.position.y + deltaY } } : node,
  )
}

export interface IfFrame {
  ifId: string
  elseId?: string
  endifId: string
}

/** Match each `if` node to its `else` (if any) and `endif` node, respecting nesting depth
 *  (same matching rule as run_scenario.py's `_validate_if_blocks`). Unmatched/orphaned
 *  if/else/endif nodes are simply omitted — malformed nesting is caught by save-time validation. */
export function computeIfFrames(nodes: Node<StepNodeData>[], edges: Edge[]): IfFrame[] {
  const frames: IfFrame[] = []
  const stack: { ifId: string; elseId?: string }[] = []
  for (const node of orderNodes(nodes, edges)) {
    if (node.data.action === "if") {
      stack.push({ ifId: node.id })
    } else if (node.data.action === "else" && stack.length > 0) {
      stack[stack.length - 1].elseId = node.id
    } else if (node.data.action === "endif" && stack.length > 0) {
      const top = stack.pop()!
      frames.push({ ifId: top.ifId, elseId: top.elseId, endifId: node.id })
    }
  }
  return frames
}

export interface VariableSummary {
  name: string
  detail: string
}

// Fixed output format each set_*_variable date action produces, for display in describeVariable.
const DATE_VARIABLE_FORMATS: Record<string, string> = {
  set_year_month_variable: "YYYYMM",
  set_year_month_day_variable: "YYYY/MM/DD",
  set_month_start_variable: "YYYY/MM/01",
  set_month_end_variable: "YYYY/MM/DD (month end)",
}

/** Human-readable summary of what a set_variable/concat_variable/set_*_variable/get_excel_value node assigns. */
function describeVariable(action: string, params: Record<string, unknown>): string {
  if (action === "set_variable" || action === "concat_variable") {
    return params.value === undefined ? "" : String(params.value)
  }
  if (action === "get_excel_value") {
    return params.cell === undefined ? "" : `Excel!${String(params.cell)}`
  }
  const parts = [DATE_VARIABLE_FORMATS[action] ?? action]
  if (params.days_offset) parts.push(`days_offset: ${params.days_offset}`)
  if (params.months_offset) parts.push(`months_offset: ${params.months_offset}`)
  return parts.join(", ")
}

/** List the variables a scenario declares (set_variable/concat_variable/set_*_variable/
 *  get_excel_value/load_table steps), in execution order. Every other action declares exactly
 *  one variable per node; load_table declares one per imported column (from its `columns`
 *  metadata, populated by the "取り込み" import flow — the column names aren't otherwise known
 *  until a loop_table block referencing it actually runs), so this flatMaps instead of mapping. */
export function listVariables(nodes: Node<StepNodeData>[], edges: Edge[]): VariableSummary[] {
  return orderNodes(nodes, edges).flatMap((node): VariableSummary[] => {
    if (node.data.action === "load_table") {
      const tableName = typeof node.data.params.name === "string" ? node.data.params.name : ""
      const columns = Array.isArray(node.data.params.columns) ? node.data.params.columns : []
      return columns
        .filter((c): c is string => typeof c === "string" && c.length > 0)
        .map((column) => ({ name: column, detail: `table: ${tableName} (value set per loop row)` }))
    }
    if (
      node.data.action !== "set_variable" &&
      node.data.action !== "concat_variable" &&
      node.data.action !== "get_excel_value" &&
      !(node.data.action in DATE_VARIABLE_FORMATS)
    ) {
      return []
    }
    const name = typeof node.data.params.name === "string" ? node.data.params.name : ""
    return name ? [{ name, detail: describeVariable(node.data.action, node.data.params) }] : []
  })
}

/** List the table names declared by load_table steps, in execution order — used to populate a
 *  loop's "table" dropdown (see LoopFrameNode). */
export function listTables(nodes: Node<StepNodeData>[], edges: Edge[]): string[] {
  return orderNodes(nodes, edges)
    .filter((node) => node.data.action === "load_table")
    .map((node) => (typeof node.data.params.name === "string" ? node.data.params.name : ""))
    .filter((name) => name.length > 0)
}

export function flowToSteps(nodes: Node<StepNodeData>[], edges: Edge[]): ScenarioStep[] {
  return orderNodes(nodes, edges).map((node) => ({
    action: node.data.action,
    ...(node.data.group ? { group: node.data.group } : {}),
    ...(node.data.loop
      ? node.data.loopTable
        ? { loop: node.data.loop, loop_table: node.data.loopTable }
        : { loop: node.data.loop, loop_count: node.data.loopCount ?? 1 }
      : {}),
    ...node.data.params,
  }))
}
