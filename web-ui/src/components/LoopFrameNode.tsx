import { useEffect, useRef, useState } from "react"
import { Handle, Position, type NodeProps } from "reactflow"
import { useLocale } from "../i18n/useLocale"
import { useCommitGuard } from "../lib/useCommitGuard"

export interface LoopFrameData {
  label: string
  count: number
  loopTable?: string
  tableNames: string[]
  isEditing: boolean
  onStartEdit: (label: string) => void
  onCommitEdit: (oldLabel: string, newLabel: string, newCount: number, newTable: string | null) => void
  onCancelEdit: () => void
}

type Mode = "count" | "table"

export default function LoopFrameNode({ data }: NodeProps<LoopFrameData>) {
  const { t } = useLocale()
  const [label, setLabel] = useState(data.label)
  const [count, setCount] = useState(String(data.count))
  const [mode, setMode] = useState<Mode>(data.loopTable ? "table" : "count")
  const [table, setTable] = useState(data.loopTable ?? "")
  const labelRef = useRef<HTMLInputElement>(null)
  const { cancel, commitUnlessCancelled } = useCommitGuard()

  useEffect(() => {
    if (data.isEditing) {
      setLabel(data.label)
      setCount(String(data.count))
      setMode(data.loopTable ? "table" : "count")
      setTable(data.loopTable ?? "")
      labelRef.current?.focus()
      labelRef.current?.select()
    }
  }, [data.isEditing, data.label, data.count, data.loopTable])

  const hiddenHandleStyle = { opacity: 0, pointerEvents: "none" as const }
  const commit = () =>
    data.onCommitEdit(data.label, label, Number(count) || 1, mode === "table" ? table.trim() || null : null)

  if (data.isEditing) {
    // Table names the dropdown offers: every load_table step in the scenario, plus the
    // currently-set table even if it's no longer one of them (e.g. its load_table step was
    // since removed/renamed) — so that value isn't silently dropped just by opening this editor.
    const tableOptions =
      table && !data.tableNames.includes(table) ? [table, ...data.tableNames] : data.tableNames

    return (
      <div className="loop-frame">
        <Handle type="target" position={Position.Top} isConnectable={false} style={hiddenHandleStyle} />
        <Handle type="source" position={Position.Bottom} isConnectable={false} style={hiddenHandleStyle} />
        <div
          className="loop-frame-edit-row loop-frame-edit-column nodrag nopan"
          onBlur={(e) => {
            if (!e.currentTarget.contains(e.relatedTarget as globalThis.Node)) commitUnlessCancelled(commit)
          }}
        >
          <input
            ref={labelRef}
            className="loop-frame-label-input"
            aria-label={t("loopLabelInput")}
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") e.currentTarget.blur()
              if (e.key === "Escape") cancel(data.onCancelEdit)
            }}
          />
          <div className="loop-frame-mode-row">
            <label>
              <input type="radio" checked={mode === "count"} onChange={() => setMode("count")} />
              {t("loopModeCount")}
            </label>
            <label>
              <input type="radio" checked={mode === "table"} onChange={() => setMode("table")} />
              {t("loopModeTable")}
            </label>
          </div>
          {mode === "count" ? (
            <input
              type="number"
              min={1}
              className="loop-frame-count-input"
              aria-label={t("loopCountInput")}
              value={count}
              onChange={(e) => setCount(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") e.currentTarget.blur()
                if (e.key === "Escape") cancel(data.onCancelEdit)
              }}
            />
          ) : (
            <select className="loop-frame-table-select" aria-label={t("loopTableSelect")} value={table} onChange={(e) => setTable(e.target.value)}>
              <option value="" disabled>
                {t("loopTablePlaceholder")}
              </option>
              {tableOptions.map((name) => (
                <option key={name} value={name}>
                  {name}
                </option>
              ))}
            </select>
          )}
        </div>
      </div>
    )
  }

  return (
    <div className="loop-frame">
      <Handle type="target" position={Position.Top} isConnectable={false} style={hiddenHandleStyle} />
      <Handle type="source" position={Position.Bottom} isConnectable={false} style={hiddenHandleStyle} />
      <div
        className="loop-frame-label nopan"
        role="button"
        tabIndex={0}
        aria-label={`${t("editLoopLabel")}: ${data.label}`}
        aria-describedby="flow-keyboard-help"
        onDoubleClick={() => data.onStartEdit(data.label)}
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault()
            data.onStartEdit(data.label)
          }
        }}
      >
        <span className="frame-kind-label">{t("loopFramePrefix")}</span> {data.label} × {data.loopTable ?? data.count}
      </div>
    </div>
  )
}
