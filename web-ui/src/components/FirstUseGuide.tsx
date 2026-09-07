import { useEffect, useState } from "react"
import { fetchEnvironmentStatus, type EnvironmentStatus } from "../api/scenarioApi"
import { useLocale } from "../i18n/useLocale"
import { readStorage, writeStorage } from "../lib/storage"
import { recordDomSetupCompleted, recordFirstUseStarted } from "../lib/usageMetrics"

const DISMISSED_KEY = "passoflow-first-use-guide-dismissed"

function wasDismissed(): boolean {
  return readStorage(DISMISSED_KEY) === "true"
}

export default function FirstUseGuide() {
  const { t } = useLocale()
  const [visible, setVisible] = useState(() => !wasDismissed())
  const [domReady, setDomReady] = useState<boolean | null>(null)
  const [playwrightReady, setPlaywrightReady] = useState<boolean | null>(null)
  const [desktopStatus, setDesktopStatus] = useState<EnvironmentStatus["desktop"] | null>(null)

  useEffect(() => {
    recordFirstUseStarted()
    let active = true
    fetchEnvironmentStatus().then(({ dom_browser, desktop }) => {
      if (!active) return
      setDomReady(dom_browser.chromium)
      setPlaywrightReady(dom_browser.playwright)
      setDesktopStatus(desktop)
      if (dom_browser.chromium) recordDomSetupCompleted()
    }).catch(() => {
      if (active) setDomReady(null)
    })
    return () => { active = false }
  }, [])

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
      <span className="first-use-guide-note">{t("firstUseGuideSetup")}</span>
      {domReady === true && <span className="first-use-guide-setup-status ready">{t("firstUseGuideDomReady")}</span>}
      {domReady === false && !playwrightReady && <span className="first-use-guide-setup-status warning">{t("firstUseGuidePlaywrightMissing")}</span>}
      {domReady === false && playwrightReady === true && <span className="first-use-guide-setup-status warning">{t("firstUseGuideChromiumMissing")}</span>}
      {desktopStatus && <span className={`first-use-guide-setup-status ${desktopStatus.capture === "ready" ? "ready" : "warning"}`}>
        {desktopStatus.capture === "ready" ? t("firstUseGuideDesktopReady") : t("firstUseGuideDesktopNeedsAttention")}
      </span>}
      {desktopStatus?.native && <details className="first-use-guide-native">
        <summary>{t("firstUseGuideNativeDiagnostics")}</summary>
        <span>{desktopStatus.native.platform} · {desktopStatus.native.supported ? t("firstUseGuideNativeSupported") : t("firstUseGuideNativeUnsupported")}</span>
        {desktopStatus.native.reason && <code>{desktopStatus.native.reason}</code>}
      </details>}
      <button type="button" className="first-use-guide-close" onClick={dismiss}>
        {t("firstUseGuideDismiss")}
      </button>
    </aside>
  )
}
