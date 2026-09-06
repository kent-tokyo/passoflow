import { Handle, Position, type NodeProps } from "reactflow"
import { useIsRunningNode } from "../context/useIsRunningNode"
import { useActionLabel } from "../i18n/useActionLabel"
import { useLocale } from "../i18n/useLocale"
import type { StepNodeData } from "../types/scenario"

function summarizeArray(value: unknown[]): string {
  if (value.length === 0) return "[]"
  const [first] = value
  if (typeof first !== "string") return `[${value.length}]`
  const basename = first.split("/").pop() ?? first
  return value.length > 1 ? `${basename} (+${value.length - 1})` : basename
}

const PARAMETER_PRIORITY: Record<string, string[]> = {
  activate_window: ["title_contains", "retry"],
  open_url: ["url"],
  browser_navigate: ["url", "timeout_ms"],
  browser_click: ["selector", "timeout_ms"],
  browser_fill: ["selector", "text"],
  browser_wait_for: ["selector", "state"],
  call_scenario: ["path"],
  click_image: ["images", "click_type", "position", "region", "retry"],
  concat_variable: ["name", "value"],
  copy_file: ["path", "destination"],
  get_excel_value: ["name", "cell", "path"],
  if: ["variable", "equals", "last_step"],
  launch_app: ["path"],
  load_table: ["name", "path", "sheet"],
  move_file: ["path", "destination"],
  move_mouse_to_image: ["images", "position", "region", "retry"],
  paste_variable: ["name"],
  repeat: ["path", "count"],
  set_clipboard: ["text"],
  set_excel_value: ["path", "cell", "value"],
  set_variable: ["name", "value"],
  sort_excel_range: ["range", "key_cell", "order"],
  type_text: ["text"],
  wait: ["ms"],
}

function summarizeParams(action: string, params: Record<string, unknown>): string {
  const entries = Object.entries(params).filter(([key]) => key !== "note" && key !== "title")
  const priority = PARAMETER_PRIORITY[action] ?? []
  entries.sort(([a], [b]) => {
    const ai = priority.indexOf(a)
    const bi = priority.indexOf(b)
    return (ai < 0 ? priority.length : ai) - (bi < 0 ? priority.length : bi)
  })
  return entries
    .slice(0, 2)
    .map(([key, value]) => `${key}: ${Array.isArray(value) ? summarizeArray(value) : String(value)}`)
    .join(" · ")
}

const BRANCH_MARKER_ACTIONS = new Set(["if", "else", "endif"])

export default function ActionNode({ id, data, selected }: NodeProps<StepNodeData>) {
  const note = data.params.note
  const customTitle = typeof data.params.title === "string" ? data.params.title : ""
  const label = useActionLabel(data.action)
  const isRunning = useIsRunningNode(id)
  const isBranchMarker = BRANCH_MARKER_ACTIONS.has(data.action)
  const { t } = useLocale()
  const accessibleLabel = data.flowStep
    ? t("flowStepLabel", { step: String(data.flowStep), label: customTitle || label })
    : customTitle || label
  return (
    <div
      className={`action-node${isBranchMarker ? " action-node-branch" : ""}${selected ? " selected" : ""}${isRunning ? " running" : ""}`}
      role="button"
      tabIndex={0}
      aria-label={`${accessibleLabel}${isRunning ? ` (${t("running")})` : ""}`}
      aria-describedby="flow-keyboard-help"
      aria-pressed={selected}
      aria-keyshortcuts="ArrowLeft ArrowRight ArrowUp ArrowDown Alt+ArrowLeft Alt+ArrowRight Alt+ArrowUp Alt+ArrowDown Shift+Alt+ArrowLeft Shift+Alt+ArrowRight Shift+Alt+ArrowUp Shift+Alt+ArrowDown"
      onKeyDown={(event) => {
        if (event.altKey && ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) {
          event.preventDefault()
          const step = event.shiftKey ? 20 : 5
          const delta = {
            ArrowLeft: [-step, 0],
            ArrowRight: [step, 0],
            ArrowUp: [0, -step],
            ArrowDown: [0, step],
          }[event.key]
          if (!delta) return
          data.onNudgeNode?.(id, delta[0], delta[1])
          return
        }
        if (!event.altKey && !event.ctrlKey && !event.metaKey && ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) {
          event.preventDefault()
          const direction = event.key.slice(5).toLowerCase() as "left" | "right" | "up" | "down"
          data.onFocusAdjacentNode?.(id, direction)
          return
        }
        if (event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey)) {
          event.preventDefault()
          const rect = event.currentTarget.getBoundingClientRect()
          data.onRequestContextMenu?.(rect.left + rect.width / 2, rect.top + rect.height / 2)
          return
        }
        if (event.key !== "Enter" && event.key !== " ") return
        event.preventDefault()
        event.currentTarget.click()
      }}
    >
      <Handle type="target" position={Position.Top} />
      {isRunning && (
        <span className="action-node-running-badge" title={t("running")}>
          ▶
        </span>
      )}
      {typeof note === "string" && note && (
        <span className="action-node-note-icon" title={note}>
          📝
        </span>
      )}
      <div className="action-node-title" title={data.action}>
        {customTitle || label}
      </div>
      {customTitle && <div className="action-node-type">{label}</div>}
      <div className="action-node-summary">{summarizeParams(data.action, data.params)}</div>
      <Handle type="source" position={Position.Bottom} />
    </div>
  )
}
