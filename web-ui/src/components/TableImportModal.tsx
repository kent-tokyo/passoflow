import { useState } from "react"
import { importTable, pickTableFile } from "../api/scenarioApi"
import { useLocale } from "../i18n/useLocale"
import Modal from "./Modal"

export interface TableImportResult {
  path: string
  sheet: string
  name: string
  columns: string[]
  rows: Record<string, string>[]
}

interface Props {
  existingNames: string[]
  onImport: (result: TableImportResult) => void
  onClose: () => void
}

/** Derive a default table name from the file's base filename, e.g. "C:\data\売上.xlsx" -> "売上",
 *  disambiguated against existingNames the same way loop/group default labels are. */
function defaultTableName(path: string, existingNames: string[]): string {
  const base = path.split(/[/\\]/).pop() ?? path
  const stem = base.replace(/\.[^.]+$/, "") || "table"
  if (!existingNames.includes(stem)) return stem
  let i = 2
  while (existingNames.includes(`${stem}${i}`)) i++
  return `${stem}${i}`
}

export default function TableImportModal({ existingNames, onImport, onClose }: Props) {
  const { t } = useLocale()
  const [path, setPath] = useState("")
  const [sheet, setSheet] = useState("")
  const [loading, setLoading] = useState(false)
  const [browsing, setBrowsing] = useState(false)
  const [error, setError] = useState("")

  const handleBrowse = async () => {
    setBrowsing(true)
    setError("")
    try {
      const picked = await pickTableFile()
      if (picked) setPath(picked)
    } catch {
      setError(t("tableImportBrowseFailed"))
    } finally {
      setBrowsing(false)
    }
  }

  const handleSubmit = async () => {
    if (!path.trim()) return
    setLoading(true)
    setError("")
    try {
      const { columns, rows } = await importTable(path.trim(), sheet)
      onImport({ path: path.trim(), sheet: sheet.trim(), name: defaultTableName(path.trim(), existingNames), columns, rows })
      onClose()
    } catch {
      setError(t("tableImportFailed"))
    } finally {
      setLoading(false)
    }
  }

  return (
    <Modal title={t("tableImportTitle")} closeLabel={t("close")} onClose={onClose} busy={loading}>
      <label className="param-field">
        {t("tableImportPath")}
        <div className="param-field-row">
          <input value={path} onChange={(e) => setPath(e.target.value)} disabled={loading || browsing} autoFocus />
          <button type="button" onClick={() => void handleBrowse()} disabled={loading || browsing}>
            {browsing ? t("tableImportBrowsing") : t("tableImportBrowse")}
          </button>
        </div>
      </label>
      <label className="param-field">
        {t("tableImportSheet")}
        <input value={sheet} onChange={(e) => setSheet(e.target.value)} disabled={loading} />
      </label>
      {error && <div className="ai-suggest-error" role="alert">{error}</div>}
      <div className="ai-suggest-actions">
        <button onClick={onClose} disabled={loading}>
          {t("cancel")}
        </button>
        <button
          className="ai-suggest-primary"
          onClick={() => void handleSubmit()}
          disabled={loading || browsing || !path.trim()}
        >
          {loading ? t("tableImportLoading") : t("tableImportSubmit")}
        </button>
      </div>
    </Modal>
  )
}
