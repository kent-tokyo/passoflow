import { useEffect, useRef, useState } from "react"
import { suggestAction, type ActionSuggestion } from "../api/scenarioApi"
import { useLocale } from "../i18n/useLocale"
import type { ActionSchema } from "../types/scenario"
import Modal from "./Modal"

interface Props {
  action: string
  params: Record<string, unknown>
  schemas: ActionSchema[]
  onApply: (action: string, params: Record<string, unknown>) => void
  onClose: () => void
}

function formatParams(params: Record<string, unknown>): string {
  const entries = Object.entries(params).filter(([key]) => key !== "note" && key !== "title")
  return entries.map(([key, value]) => `${key}: ${Array.isArray(value) ? value.join(", ") : String(value)}`).join("\n")
}

export default function AiSuggestModal({ action, params, schemas, onApply, onClose }: Props) {
  const { locale, t } = useLocale()
  const [instruction, setInstruction] = useState("")
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState("")
  const [suggestion, setSuggestion] = useState<ActionSuggestion | null>(null)
  const mountedRef = useRef(true)
  useEffect(() => () => {
    mountedRef.current = false
  }, [])

  const actionLabel = (name: string) => schemas.find((s) => s.action === name)?.labels[locale] ?? name

  const handleSubmit = async () => {
    setLoading(true)
    setError("")
    try {
      const result = await suggestAction(action, params, instruction)
      if (mountedRef.current) setSuggestion(result)
    } catch {
      if (mountedRef.current) setError(t("aiSuggestFailed"))
    } finally {
      if (mountedRef.current) setLoading(false)
    }
  }

  const handleApply = () => {
    if (!suggestion) return
    // The model is told to carry these over, but default to preserving them if it drops them anyway.
    const preserved: Record<string, unknown> = {}
    if (params.note !== undefined && suggestion.params.note === undefined) preserved.note = params.note
    if (params.title !== undefined && suggestion.params.title === undefined) preserved.title = params.title
    // load_table's `columns` is UI-only metadata (populated by the "取り込み" import flow, not
    // part of ACTION_SCHEMA, so the model's catalog doesn't even know it exists) — always
    // preserve it verbatim rather than relying on the model to carry over a field it was never
    // told about, as long as the action itself isn't changing.
    if (action === "load_table" && suggestion.action === "load_table" && params.columns !== undefined) {
      preserved.columns = params.columns
    }
    onApply(suggestion.action, { ...preserved, ...suggestion.params })
    onClose()
  }

  const paramsText = suggestion ? formatParams(suggestion.params) : ""

  return (
    <Modal title={t("aiSuggestTitle", { action: actionLabel(action) })} closeLabel={t("close")} onClose={onClose} busy={loading}>
      {!suggestion ? (
        <>
          <textarea
            className="ai-suggest-instruction"
            value={instruction}
            onChange={(e) => setInstruction(e.target.value)}
            placeholder={t("aiSuggestPlaceholder")}
            rows={3}
            disabled={loading}
            autoFocus
          />
          {error && <div className="ai-suggest-error" role="alert">{error}</div>}
          <div className="ai-suggest-actions">
            <button
              className="ai-suggest-clear"
              onClick={() => setInstruction("")}
              disabled={loading || !instruction}
            >
              {t("aiSuggestClear")}
            </button>
            <button onClick={onClose} disabled={loading}>
              {t("cancel")}
            </button>
            <button className="ai-suggest-primary" onClick={() => void handleSubmit()} disabled={loading}>
              {loading ? t("chatThinking") : t("aiSuggestSubmit")}
            </button>
          </div>
        </>
      ) : (
        <>
          {suggestion.action !== action && (
            <div className="ai-suggest-action-change">
              {t("aiSuggestActionChanged", { from: actionLabel(action), to: actionLabel(suggestion.action) })}
            </div>
          )}
          {paramsText && <pre className="ai-suggest-params">{paramsText}</pre>}
          <p className="ai-suggest-explanation">{suggestion.explanation}</p>
          <div className="ai-suggest-actions">
            <button onClick={() => setSuggestion(null)}>{t("aiSuggestDiscard")}</button>
            <button className="ai-suggest-primary" onClick={handleApply}>
              {t("aiSuggestApply")}
            </button>
          </div>
        </>
      )}
    </Modal>
  )
}
