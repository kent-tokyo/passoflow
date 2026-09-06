import { type KeyboardEvent } from "react"
import { useLocale } from "../i18n/useLocale"
import { useWindowDragResize } from "../hooks/useWindowDragResize"

interface Props {
  side: "palette" | "parameter"
  width: number
  minWidth: number
  maxWidth: number
  onWidthChange: (width: number) => void
}

const KEYBOARD_STEP = 16
const LARGE_KEYBOARD_STEP = 48

export default function PanelResizeHandle({ side, width, minWidth, maxWidth, onWidthChange }: Props) {
  const { t } = useLocale()
  const label = side === "palette" ? t("resizeActionPalette") : t("resizeParameterPanel")
  const widthLabel = side === "palette" ? t("actionPaletteWidth") : t("parameterPanelWidth")
  const clamp = (value: number) => Math.min(maxWidth, Math.max(minWidth, value))
  const startResize = useWindowDragResize({
    onMove: (start, current) => {
      const direction = side === "palette" ? 1 : -1
      onWidthChange(clamp(width + (current.x - start.x) * direction))
    },
  })

  const resizeByKeyboard = (event: KeyboardEvent<HTMLDivElement>, direction: number) => {
    event.preventDefault()
    event.stopPropagation()
    const step = event.shiftKey ? LARGE_KEYBOARD_STEP : KEYBOARD_STEP
    const sideDirection = side === "palette" ? 1 : -1
    onWidthChange(clamp(width + step * direction * sideDirection))
  }

  return (
    <div
      className={`panel-resize-handle panel-resize-handle-${side} nopan nodrag`}
      role="separator"
      tabIndex={0}
      aria-orientation="vertical"
      aria-valuemin={minWidth}
      aria-valuemax={maxWidth}
      aria-valuenow={Math.round(width)}
      aria-valuetext={widthLabel.replace("{width}", String(Math.round(width)))}
      aria-label={label}
      title={label}
      onMouseDown={(event) => startResize(event, { x: event.clientX, y: event.clientY })}
      onKeyDown={(event) => {
        if (event.key === "ArrowLeft") resizeByKeyboard(event, -1)
        else if (event.key === "ArrowRight") resizeByKeyboard(event, 1)
        else if (event.key === "Home") {
          event.preventDefault()
          event.stopPropagation()
          onWidthChange(minWidth)
        } else if (event.key === "End") {
          event.preventDefault()
          event.stopPropagation()
          onWidthChange(maxWidth)
        }
      }}
    >
      <span aria-hidden="true" />
    </div>
  )
}
