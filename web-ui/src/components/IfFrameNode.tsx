import { Handle, Position, type NodeProps } from "reactflow"
import { useLocale } from "../i18n/useLocale"

export interface IfFrameData {
  ifId: string
  variable: string
  equals?: string
  last_step?: string
  dividerTop?: number
}

export default function IfFrameNode({ data }: NodeProps<IfFrameData>) {
  const { t } = useLocale()
  const hiddenHandleStyle = { opacity: 0, pointerEvents: "none" as const }
  const condition = data.last_step
    ? `last step == ${data.last_step}`
    : data.variable
      ? data.equals
        ? `${data.variable} == ${data.equals}`
        : data.variable
      : "..."

  return (
    <div
      className="if-frame"
      role="group"
      aria-label={`${t("ifFramePrefix")}: ${condition}; ${t("trueBranch")}${data.dividerTop != null ? `, ${t("falseBranch")}` : ""}`}
    >
      <Handle type="target" position={Position.Top} isConnectable={false} style={hiddenHandleStyle} />
      <Handle type="source" position={Position.Bottom} isConnectable={false} style={hiddenHandleStyle} />
      <div className="if-frame-label nopan">{t("ifFramePrefix")}: {condition}</div>
      <span className="if-frame-branch-label if-frame-branch-true">{t("trueBranch")}</span>
      {data.dividerTop != null && (
        <>
          <div className="if-frame-divider" style={{ top: data.dividerTop }} />
          <span className="if-frame-branch-label if-frame-branch-false" style={{ top: data.dividerTop + 6 }}>
            {t("falseBranch")}
          </span>
        </>
      )}
    </div>
  )
}
