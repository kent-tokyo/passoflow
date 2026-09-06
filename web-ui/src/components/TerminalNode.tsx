import { Handle, Position, type NodeProps } from "reactflow"
import { useIsRunningNode } from "../context/useIsRunningNode"
import { useActionLabel } from "../i18n/useActionLabel"
import { useLocale } from "../i18n/useLocale"
import type { StepNodeData } from "../types/scenario"

export default function TerminalNode({ id, data, selected }: NodeProps<StepNodeData>) {
  const isStart = data.action === "start"
  const label = useActionLabel(data.action)
  const isRunning = useIsRunningNode(id)
  const { t } = useLocale()
  const accessibleLabel = data.flowStep
    ? t("flowStepLabel", { step: String(data.flowStep), label })
    : label
  return (
    <div
      className={`terminal-node terminal-node-${data.action}${selected ? " selected" : ""}${
        isRunning ? " running" : ""
      }`}
      role="button"
      tabIndex={0}
      aria-label={`${accessibleLabel}${isRunning ? ` (${t("running")})` : ""}`}
      aria-describedby="flow-keyboard-help"
      aria-pressed={selected}
      aria-keyshortcuts="ArrowLeft ArrowRight ArrowUp ArrowDown Alt+ArrowLeft Alt+ArrowRight Alt+ArrowUp Alt+ArrowDown Shift+Alt+ArrowLeft Shift+Alt+ArrowRight Shift+Alt+ArrowUp Shift+Alt+ArrowDown ContextMenu Shift+F10"
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
          if (delta) data.onNudgeNode?.(id, delta[0], delta[1])
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
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault()
          event.currentTarget.click()
        }
      }}
    >
      {!isStart && <Handle type="target" position={Position.Top} />}
      <span>{label}</span>
      {isStart && <Handle type="source" position={Position.Bottom} />}
    </div>
  )
}
