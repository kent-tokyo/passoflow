import { useState } from "react"
import { useLocale } from "../i18n/useLocale"
import { readStorage, writeStorage } from "../lib/storage"

const DISMISSED_KEY = "passoflow-first-use-guide-dismissed"

function wasDismissed(): boolean {
  return readStorage(DISMISSED_KEY) === "true"
}

export default function FirstUseGuide() {
  const { t } = useLocale()
  const [visible, setVisible] = useState(() => !wasDismissed())

  if (!visible) return null

  const dismiss = () => {
    writeStorage(DISMISSED_KEY, "true")
    setVisible(false)
  }

  return (
    <aside className="first-use-guide" aria-label={t("firstUseGuideTitle")}>
      <div className="first-use-guide-copy">
        <strong>{t("firstUseGuideTitle")}</strong>
        <span>{t("firstUseGuideDescription")}</span>
      </div>
      <ol className="first-use-guide-steps">
        <li>{t("firstUseGuideStepAdd")}</li>
        <li>{t("firstUseGuideStepConfigure")}</li>
        <li>{t("firstUseGuideStepRun")}</li>
      </ol>
      <button type="button" className="first-use-guide-close" onClick={dismiss}>
        {t("firstUseGuideDismiss")}
      </button>
    </aside>
  )
}
