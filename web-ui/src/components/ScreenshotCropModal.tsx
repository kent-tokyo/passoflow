import { useEffect, useRef, useState, type MouseEvent as ReactMouseEvent } from "react"
import { captureScreenshot, uploadScenarioImage } from "../api/scenarioApi"
import { useLocale } from "../i18n/useLocale"
import Modal from "./Modal"

interface Props {
  scenarioFilename: string
  positionOptions?: string[]
  initialPosition?: string
  onCaptured: (path: string, position?: string) => void
  onClose: () => void
}

type Phase = "countdown" | "loading" | "crop" | "error"

const COUNTDOWN_SECONDS = 3

interface Rect {
  x: number
  y: number
  width: number
  height: number
}

export default function ScreenshotCropModal({
  scenarioFilename,
  positionOptions = [],
  initialPosition = "center",
  onCaptured,
  onClose,
}: Props) {
  const { t } = useLocale()
  const [phase, setPhase] = useState<Phase>("countdown")
  const [secondsLeft, setSecondsLeft] = useState(COUNTDOWN_SECONDS)
  const [imageSrc, setImageSrc] = useState<string | null>(null)
  const [selection, setSelection] = useState<Rect | null>(null)
  const [filename, setFilename] = useState("")
  const [position, setPosition] = useState(initialPosition)
  const imgRef = useRef<HTMLImageElement>(null)
  const objectUrlRef = useRef<string | null>(null)

  const capture = async () => {
    setPhase("loading")
    try {
      const blob = await captureScreenshot()
      if (objectUrlRef.current) URL.revokeObjectURL(objectUrlRef.current)
      const url = URL.createObjectURL(blob)
      objectUrlRef.current = url
      setImageSrc(url)
      setSelection(null)
      setPhase("crop")
    } catch {
      setPhase("error")
    }
  }

  useEffect(() => {
    if (phase !== "countdown") return
    if (secondsLeft <= 0) {
      void capture()
      return
    }
    const timer = window.setTimeout(() => setSecondsLeft((s) => s - 1), 1000)
    return () => window.clearTimeout(timer)
  }, [phase, secondsLeft])

  useEffect(() => {
    return () => {
      if (objectUrlRef.current) URL.revokeObjectURL(objectUrlRef.current)
    }
  }, [])

  const retry = () => {
    setSecondsLeft(COUNTDOWN_SECONDS)
    setPhase("countdown")
  }

  const selectFullImage = () => {
    const img = imgRef.current
    if (!img || img.naturalWidth < 1 || img.naturalHeight < 1) return
    setSelection({ x: 0, y: 0, width: img.naturalWidth, height: img.naturalHeight })
  }

  const toNaturalPoint = (clientX: number, clientY: number) => {
    const img = imgRef.current
    if (!img) return { x: 0, y: 0 }
    const rect = img.getBoundingClientRect()
    const scaleX = img.naturalWidth / rect.width
    const scaleY = img.naturalHeight / rect.height
    return {
      x: Math.min(Math.max(0, (clientX - rect.left) * scaleX), img.naturalWidth),
      y: Math.min(Math.max(0, (clientY - rect.top) * scaleY), img.naturalHeight),
    }
  }

  const onMouseDown = (e: ReactMouseEvent) => {
    const start = toNaturalPoint(e.clientX, e.clientY)
    setSelection({ x: start.x, y: start.y, width: 0, height: 0 })

    const onMove = (moveEvent: MouseEvent) => {
      const point = toNaturalPoint(moveEvent.clientX, moveEvent.clientY)
      setSelection({
        x: Math.min(start.x, point.x),
        y: Math.min(start.y, point.y),
        width: Math.abs(point.x - start.x),
        height: Math.abs(point.y - start.y),
      })
    }
    const onUp = () => {
      window.removeEventListener("mousemove", onMove)
      window.removeEventListener("mouseup", onUp)
    }
    window.addEventListener("mousemove", onMove)
    window.addEventListener("mouseup", onUp)
  }

  const handleSave = async () => {
    const img = imgRef.current
    if (!img || !selection || selection.width < 1 || selection.height < 1) return
    const raw = filename.trim()
    if (!raw) return
    const outputFilename = /\.png$/i.test(raw) ? raw : `${raw}.png`

    const canvas = document.createElement("canvas")
    canvas.width = Math.round(selection.width)
    canvas.height = Math.round(selection.height)
    const ctx = canvas.getContext("2d")
    if (!ctx) return
    ctx.drawImage(img, selection.x, selection.y, selection.width, selection.height, 0, 0, canvas.width, canvas.height)

    canvas.toBlob(async (blob) => {
      if (!blob) return
      const file = new File([blob], outputFilename, { type: "image/png" })
      try {
        const { path } = await uploadScenarioImage(scenarioFilename, file)
        onCaptured(path, positionOptions.length > 0 ? position : undefined)
        onClose()
      } catch {
        setPhase("error")
      }
    }, "image/png")
  }

  const displayRect = (() => {
    const img = imgRef.current
    if (!img || !selection) return null
    const rect = img.getBoundingClientRect()
    const scaleX = rect.width / img.naturalWidth
    const scaleY = rect.height / img.naturalHeight
    return {
      left: selection.x * scaleX,
      top: selection.y * scaleY,
      width: selection.width * scaleX,
      height: selection.height * scaleY,
    }
  })()

  const controlsStyle = (() => {
    if (!displayRect || !imgRef.current) return undefined
    const imageWidth = imgRef.current.getBoundingClientRect().width
    const panelWidth = Math.min(220, Math.max(160, window.innerWidth - 32))
    const right = displayRect.left + displayRect.width + 10
    const left = right + panelWidth <= imageWidth ? right : Math.max(8, displayRect.left - panelWidth - 10)
    return { left, top: Math.max(8, displayRect.top) }
  })()

  return (
    <Modal
      title={t("screenshotCaptureTitle")}
      closeLabel={t("close")}
      onClose={onClose}
      className="screenshot-crop-modal"
      busy={phase === "loading"}
    >
      {phase === "countdown" && (
        <>
          <p className="ai-suggest-instruction">{t("screenshotCountdown", { seconds: String(secondsLeft) })}</p>
          <div className="ai-suggest-actions">
            <button onClick={onClose}>{t("cancel")}</button>
          </div>
        </>
      )}
      {phase === "loading" && <p className="ai-suggest-instruction">{t("screenshotCaptureTitle")}...</p>}
      {phase === "error" && (
        <>
          <p className="ai-suggest-error" role="alert">{t("screenshotCaptureFailed")}</p>
          <div className="ai-suggest-actions">
            <button onClick={onClose}>{t("cancel")}</button>
            <button className="ai-suggest-primary" onClick={retry}>
              {t("screenshotRetry")}
            </button>
          </div>
        </>
      )}
      {phase === "crop" && imageSrc && (
        <>
          <p className="ai-suggest-instruction">{t("screenshotSelectHint")}</p>
          <div className="screenshot-crop-image-wrapper" onMouseDown={onMouseDown}>
            <img ref={imgRef} src={imageSrc} alt={t("screenshotPreviewAlt")} draggable={false} />
            {displayRect && (
              <div
                className="screenshot-crop-selection"
                style={{
                  left: displayRect.left,
                  top: displayRect.top,
                  width: displayRect.width,
                  height: displayRect.height,
                }}
              />
            )}
            {displayRect && controlsStyle && (
              <div
                className="screenshot-crop-controls"
                style={controlsStyle}
                onMouseDown={(event) => event.stopPropagation()}
              >
                <label>
                  <span>{t("screenshotSavePrompt")}</span>
                  <input
                    value={filename}
                    onChange={(event) => setFilename(event.target.value)}
                    placeholder={t("screenshotFilenamePlaceholder")}
                    autoFocus
                  />
                </label>
                {positionOptions.length > 0 && (
                  <label>
                    <span>{t("screenshotClickPosition")}</span>
                    <select value={position} onChange={(event) => setPosition(event.target.value)}>
                      {positionOptions.map((option) => (
                        <option key={option} value={option}>
                          {option}
                        </option>
                      ))}
                    </select>
                  </label>
                )}
                <button
                  type="button"
                  className="ai-suggest-primary"
                  onClick={() => void handleSave()}
                  disabled={!filename.trim()}
                >
                  {t("screenshotSave")}
                </button>
              </div>
            )}
          </div>
          <div className="ai-suggest-actions">
            <button type="button" onClick={selectFullImage} aria-label={t("screenshotSelectFullImage")}>
              {t("screenshotSelectFullImage")}
            </button>
            <button onClick={retry}>{t("screenshotRetry")}</button>
          </div>
        </>
      )}
    </Modal>
  )
}
