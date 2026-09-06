import { useEffect, useState } from "react"
import { captureScreenshot } from "../api/scenarioApi"
import { useLocale } from "../i18n/useLocale"
import Modal from "./Modal"

interface Props {
  region: number[] | null
  regionOrigin: string
  onClose: () => void
}

export default function RegionPreviewModal({ region, regionOrigin, onClose }: Props) {
  const { t } = useLocale()
  const [imageSrc, setImageSrc] = useState<string | null>(null)
  const [failed, setFailed] = useState(false)
  const [imageSize, setImageSize] = useState<{ width: number; height: number } | null>(null)

  useEffect(() => {
    let active = true
    let objectUrl: string | null = null
    captureScreenshot().then((blob) => {
      if (!active) return
      objectUrl = URL.createObjectURL(blob)
      setImageSrc(objectUrl)
    }).catch(() => {
      if (active) setFailed(true)
    })
    return () => {
      active = false
      if (objectUrl) URL.revokeObjectURL(objectUrl)
    }
  }, [])

  const validRegion = region && region.length === 4 && region[2] > 0 && region[3] > 0 ? region : null
  const showOverlay = validRegion && regionOrigin === "screen" && imageSize !== null

  return (
    <Modal title={t("regionPreviewTitle")} closeLabel={t("close")} onClose={onClose} className="region-preview-modal">
      {failed && <p className="ai-suggest-error" role="alert">{t("regionPreviewFailed")}</p>}
      {!failed && !imageSrc && <p className="ai-suggest-instruction">{t("regionPreviewLoading")}</p>}
      {imageSrc && (
        <>
          <p className="ai-suggest-instruction">
            {showOverlay ? t("regionPreviewHint") : t("regionPreviewActiveWindowHint")}
          </p>
          <div className="region-preview-image-wrapper">
            <img
              src={imageSrc}
              alt={t("regionPreviewAlt")}
              onLoad={(event) => setImageSize({ width: event.currentTarget.naturalWidth, height: event.currentTarget.naturalHeight })}
            />
            {showOverlay && (
              <div
                className="region-preview-selection"
                style={{
                  left: `${(validRegion[0] / imageSize.width) * 100}%`,
                  top: `${(validRegion[1] / imageSize.height) * 100}%`,
                  width: `${(validRegion[2] / imageSize.width) * 100}%`,
                  height: `${(validRegion[3] / imageSize.height) * 100}%`,
                }}
              />
            )}
          </div>
        </>
      )}
      <div className="ai-suggest-actions">
        <button type="button" onClick={onClose}>{t("close")}</button>
      </div>
    </Modal>
  )
}
