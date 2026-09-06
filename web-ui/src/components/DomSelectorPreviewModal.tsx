import { useState } from "react"
import { previewDomSelector } from "../api/scenarioApi"
import { useLocale } from "../i18n/useLocale"
import Modal from "./Modal"

interface Props {
  initialUrl: string
  initialSelector: string
  onUseSelector: (selector: string) => void
  onClose: () => void
}

export default function DomSelectorPreviewModal({ initialUrl, initialSelector, onUseSelector, onClose }: Props) {
  const { t } = useLocale()
  const [url, setUrl] = useState(initialUrl)
  const [selector, setSelector] = useState(initialSelector)
  const [result, setResult] = useState<Awaited<ReturnType<typeof previewDomSelector>> | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const explainError = (message: string) => {
    const lower = message.toLowerCase()
    if (lower.includes("timeout")) return t("domSelectorRecoveryTimeout")
    if (lower.includes("selector") && (lower.includes("invalid") || lower.includes("not valid"))) return t("domSelectorRecoveryInvalid")
    if (lower.includes("net::") || lower.includes("navigation")) return t("domSelectorRecoveryNavigation")
    return t("domSelectorPreviewFailed", { error: message })
  }

  const runPreview = async () => {
    setBusy(true)
    setError(null)
    setResult(null)
    try {
      setResult(await previewDomSelector(url, selector))
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause)
      setError(explainError(message.replace(/^.*detail["']?:["']?\s*/i, "")))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Modal title={t("domSelectorPreviewTitle")} closeLabel={t("close")} onClose={onClose} busy={busy} className="dom-selector-preview-modal">
      <p className="ai-suggest-instruction">{t("domSelectorPreviewHint")}</p>
      <label className="param-field">
        <span>{t("domSelectorPreviewUrl")}</span>
        <input type="url" value={url} placeholder="https://example.com" onChange={(event) => setUrl(event.target.value)} />
      </label>
      <label className="param-field">
        <span>{t("domSelectorPreviewSelector")}</span>
        <input type="text" value={selector} placeholder="button[type=submit]" onChange={(event) => setSelector(event.target.value)} />
      </label>
      {error && <p className="ai-suggest-error" role="alert">{error}</p>}
      {result && (
        <div className="dom-selector-preview-result" role="status">
          <p>{t("domSelectorPreviewMatches", { count: String(result.count) })}</p>
          {result.samples.length > 0 && (
            <ul>
              {result.samples.map((sample, index) => (
                <li key={`${sample.tag}-${index}`}>
                  <code>&lt;{sample.tag}&gt;</code> {sample.text || t("domSelectorPreviewNoText")}
                  {!sample.visible && <span> ({t("domSelectorPreviewHidden")})</span>}
                  <button type="button" className="dom-selector-sample-button" onClick={() => onUseSelector(sample.selector)}>
                    {t("domSelectorCapture", { selector: sample.selector })}
                  </button>
                </li>
              ))}
            </ul>
          )}
          {result.suggested_selector && result.suggested_selector !== selector.trim() && (
            <button type="button" className="browse-button" onClick={() => onUseSelector(result.suggested_selector ?? "")}>
              {t("domSelectorUseSuggestion", { selector: result.suggested_selector })}
            </button>
          )}
          {result.count === 0 && result.repair_suggestions.length > 0 && (
            <div className="dom-selector-repair-suggestions">
              <p>{t("domSelectorRepairTitle")}</p>
              <ul>
                {result.repair_suggestions.map((suggestion) => (
                  <li key={suggestion.selector}>
                    <code>{suggestion.selector}</code> ({t("domSelectorPreviewMatches", { count: String(suggestion.count) })})
                    <button type="button" className="dom-selector-sample-button" onClick={() => onUseSelector(suggestion.selector)}>
                      {t("domSelectorUseRepair")}
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
      <div className="ai-suggest-actions">
        <button type="button" className="ai-suggest-primary" onClick={() => void runPreview()} disabled={busy || !url.trim() || !selector.trim()}>
          {busy ? t("domSelectorPreviewRunning") : t("domSelectorPreviewRun")}
        </button>
        <button type="button" onClick={onClose}>{t("close")}</button>
      </div>
    </Modal>
  )
}
