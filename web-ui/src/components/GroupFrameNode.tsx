import { useEffect, useRef, useState } from "react"
import { Handle, Position, type NodeProps } from "reactflow"
import { useLocale } from "../i18n/useLocale"
import { useCommitGuard } from "../lib/useCommitGuard"

export interface GroupFrameData {
  label: string
  isEditing: boolean
  onStartEdit: (label: string) => void
  onCommitEdit: (oldLabel: string, newValue: string) => void
  onCancelEdit: () => void
}

export default function GroupFrameNode({ data }: NodeProps<GroupFrameData>) {
  const { t } = useLocale()
  const [value, setValue] = useState(data.label)
  const inputRef = useRef<HTMLInputElement>(null)
  const { cancel, commitUnlessCancelled } = useCommitGuard()

  useEffect(() => {
    if (data.isEditing) {
      setValue(data.label)
      inputRef.current?.focus()
      inputRef.current?.select()
    }
  }, [data.isEditing, data.label])

  const hiddenHandleStyle = { opacity: 0, pointerEvents: "none" as const }

  if (data.isEditing) {
    return (
      <div className="group-frame">
        <Handle type="target" position={Position.Top} isConnectable={false} style={hiddenHandleStyle} />
        <Handle type="source" position={Position.Bottom} isConnectable={false} style={hiddenHandleStyle} />
        <input
          ref={inputRef}
          className="group-frame-label-input nodrag nopan"
          aria-label={t("editGroupLabel")}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onBlur={() => commitUnlessCancelled(() => data.onCommitEdit(data.label, value))}
          onKeyDown={(e) => {
            if (e.key === "Enter") e.currentTarget.blur()
            if (e.key === "Escape") cancel(data.onCancelEdit)
          }}
        />
      </div>
    )
  }

  return (
    <div className="group-frame">
      <Handle type="target" position={Position.Top} isConnectable={false} style={hiddenHandleStyle} />
      <Handle type="source" position={Position.Bottom} isConnectable={false} style={hiddenHandleStyle} />
      <div
        className="group-frame-label nopan"
        role="button"
        tabIndex={0}
        aria-label={`${t("editGroupLabel")}: ${data.label}`}
        aria-describedby="flow-keyboard-help"
        onDoubleClick={() => data.onStartEdit(data.label)}
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault()
            data.onStartEdit(data.label)
          }
        }}
      >
        <span className="frame-kind-label">{t("groupFramePrefix")}</span> {data.label}
      </div>
    </div>
  )
}
