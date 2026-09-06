import { useEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react"
import { useLocale } from "../i18n/useLocale"
import type { VariableSummary } from "../lib/flowConversion"
import { readStorage, writeStorage } from "../lib/storage"
import { useWindowDragResize } from "../hooks/useWindowDragResize"

const HEIGHT_STORAGE_KEY = "passoflow-execution-panel-height"
const MIN_HEIGHT = 80

function loadStoredHeight(): number {
  const stored = Number(readStorage(HEIGHT_STORAGE_KEY))
  return Number.isFinite(stored) && stored >= MIN_HEIGHT ? stored : 160
}

interface ImportedTable {
  name: string
  columns: string[]
  rows: Record<string, string>[]
  selectedRows: number[]
}

interface Props {
  logs: string[]
  running: boolean
  variables: VariableSummary[]
  importedTable: ImportedTable | null
  onImportTable: () => void
  onImportedRowsChange: (selectedRows: number[]) => void
  onStepClick: (step: number) => void
  onRerunStep: (step: number) => void
  runContext: RunContext | null
}

export interface RunContext {
  filename: string
  startedAt: string
  start?: number
  end?: number
  outcome: string
  lastCompletedStep: number | null
}

type Tab = "log" | "variables" | "importedTable"
type SortState = { col: string; dir: "asc" | "desc" } | null

const TABS: Tab[] = ["log", "variables", "importedTable"]

function compareRows(a: string, b: string): number {
  const na = Number(a)
  const nb = Number(b)
  if (a !== "" && b !== "" && Number.isFinite(na) && Number.isFinite(nb)) return na - nb
  return a.localeCompare(b)
}

function stepNumberFromLog(line: string): number | null {
  const match = line.match(/\bstep\s+(\d+)\b/i)
  return match ? Number(match[1]) : null
}

export default function ExecutionPanel({
  logs,
  running,
  variables,
  importedTable,
  onImportTable,
  onImportedRowsChange,
  onStepClick,
  onRerunStep,
  runContext,
}: Props) {
  const { t } = useLocale()
  const [tab, setTab] = useState<Tab>("log")
  const [sort, setSort] = useState<SortState>(null)
  const [checked, setChecked] = useState<Set<number>>(new Set())
  const [height, setHeight] = useState(loadStoredHeight)
  const tabRefs = useRef<Partial<Record<Tab, HTMLButtonElement>>>({})

  const selectTabWithFocus = (nextTab: Tab) => {
    setTab(nextTab)
    requestAnimationFrame(() => tabRefs.current[nextTab]?.focus())
  }

  const handleTabKeyDown = (event: ReactKeyboardEvent<HTMLButtonElement>, currentTab: Tab) => {
    const currentIndex = TABS.indexOf(currentTab)
    let nextIndex: number | null = null
    if (event.key === "ArrowRight" || event.key === "ArrowDown") nextIndex = (currentIndex + 1) % TABS.length
    if (event.key === "ArrowLeft" || event.key === "ArrowUp") nextIndex = (currentIndex - 1 + TABS.length) % TABS.length
    if (event.key === "Home") nextIndex = 0
    if (event.key === "End") nextIndex = TABS.length - 1
    if (nextIndex == null) return
    event.preventDefault()
    selectTabWithFocus(TABS[nextIndex])
  }

  const startResize = useWindowDragResize({
    onMove: (start, current) => {
      const maxHeight = window.innerHeight * 0.6
      const next = Math.min(maxHeight, Math.max(MIN_HEIGHT, height + (start.y - current.y)))
      setHeight(next)
    },
    onEnd: () => {
      setHeight((current) => {
        writeStorage(HEIGHT_STORAGE_KEY, String(current))
        return current
      })
    },
  })

  const resizeByKeyboard = (delta: number) => {
    const maxHeight = window.innerHeight * 0.6
    setHeight((current) => {
      const next = Math.min(maxHeight, Math.max(MIN_HEIGHT, current + delta))
      writeStorage(HEIGHT_STORAGE_KEY, String(next))
      return next
    })
  }

  useEffect(() => {
    setSort(null)
    setChecked(new Set(importedTable?.selectedRows ?? []))
  }, [importedTable])

  const rowOrder = useMemo(() => {
    if (!importedTable) return []
    const indices = importedTable.rows.map((_, i) => i)
    if (!sort) return indices
    const { col, dir } = sort
    const factor = dir === "asc" ? 1 : -1
    return indices.sort((a, b) => factor * compareRows(importedTable.rows[a][col] ?? "", importedTable.rows[b][col] ?? ""))
  }, [importedTable, sort])

  const toggleSort = (col: string) => {
    setSort((prev) => {
      if (!prev || prev.col !== col) return { col, dir: "asc" }
      if (prev.dir === "asc") return { col, dir: "desc" }
      return null
    })
  }

  const toggleRowChecked = (i: number) => {
    setChecked((prev) => {
      const next = new Set(prev)
      if (next.has(i)) next.delete(i)
      else next.add(i)
      return next
    })
  }

  return (
    <div className="execution-panel" style={{ height }}>
      <div
        className="execution-panel-resize-handle"
        role="separator"
        tabIndex={0}
        aria-orientation="horizontal"
        aria-valuemin={MIN_HEIGHT}
          aria-valuemax={Math.round(window.innerHeight * 0.6)}
          aria-valuenow={Math.round(height)}
          aria-valuetext={t("executionPanelHeight", { height: String(Math.round(height)) })}
          aria-label={t("resizeExecutionPanel")}
          onMouseDown={(event) => startResize(event, { x: event.clientX, y: event.clientY })}
        onKeyDown={(event) => {
          if (event.key === "ArrowUp") {
            event.preventDefault()
            resizeByKeyboard(16)
          } else if (event.key === "ArrowDown") {
            event.preventDefault()
            resizeByKeyboard(-16)
          }
        }}
      />
      <div className="execution-panel-tabs" role="tablist" aria-label={t("executionPanelTabs")}>
        <button
          type="button"
          role="tab"
          id="execution-tab-log"
          aria-controls={tab === "log" ? "execution-tabpanel-log" : undefined}
          tabIndex={tab === "log" ? 0 : -1}
          className={`execution-panel-tab${tab === "log" ? " active" : ""}`}
          aria-selected={tab === "log"}
          ref={(element) => { if (element) tabRefs.current.log = element }}
          onClick={() => selectTabWithFocus("log")}
          onKeyDown={(event) => handleTabKeyDown(event, "log")}
        >
          {t("executionLog")}
          {running ? `（${t("running")}）` : ""}
        </button>
        <button
          type="button"
          role="tab"
          id="execution-tab-variables"
          aria-controls={tab === "variables" ? "execution-tabpanel-variables" : undefined}
          tabIndex={tab === "variables" ? 0 : -1}
          className={`execution-panel-tab${tab === "variables" ? " active" : ""}`}
          aria-selected={tab === "variables"}
          ref={(element) => { if (element) tabRefs.current.variables = element }}
          onClick={() => selectTabWithFocus("variables")}
          onKeyDown={(event) => handleTabKeyDown(event, "variables")}
        >
          {t("variableList")}
        </button>
        <button
          type="button"
          role="tab"
          id="execution-tab-importedTable"
          aria-controls={tab === "importedTable" ? "execution-tabpanel-importedTable" : undefined}
          tabIndex={tab === "importedTable" ? 0 : -1}
          className={`execution-panel-tab${tab === "importedTable" ? " active" : ""}`}
          aria-selected={tab === "importedTable"}
          ref={(element) => { if (element) tabRefs.current.importedTable = element }}
          onClick={() => selectTabWithFocus("importedTable")}
          onKeyDown={(event) => handleTabKeyDown(event, "importedTable")}
        >
          {t("importedDataTab")}
        </button>
      </div>
      {tab === "log" ? (
        <div id="execution-tabpanel-log" role="tabpanel" aria-labelledby="execution-tab-log" tabIndex={0}>
          {runContext && (
            <div className="run-context" aria-label={t("runContextHeader")}>
              <div className="run-context-title">{t("runContextHeader")}</div>
              <div>{t("runContextScenario", { filename: runContext.filename })}</div>
              <div>{t("runContextStarted", { time: new Date(runContext.startedAt).toLocaleString() })}</div>
              <div>
                {runContext.start == null && runContext.end == null
                  ? t("preflightFullScope")
                  : t("preflightSelectedScope", {
                      start: String(runContext.start ?? 1),
                      end: String(runContext.end ?? "?"),
                    })}
              </div>
              <div>{t("runContextOutcome", { outcome: runContext.outcome })}</div>
              <div>
                {runContext.lastCompletedStep == null
                  ? t("runNoCompletedStep")
                  : t("runLastCompleted", { step: String(runContext.lastCompletedStep) })}
              </div>
            </div>
          )}
          <div className="log-output" role="log" aria-live="polite">
          {logs.length === 0 && <p className="panel-empty-guide">{t("executionLogEmpty")}</p>}
          {logs.map((line, index) => {
            const step = stepNumberFromLog(line)
            return step ? (
              line.includes("Failure screenshots saved") ? (
                <div className="log-line-step-row" key={`${index}-${line}`}>
                  <button
                    className="log-line log-line-step"
                    type="button"
                    onClick={() => onStepClick(step)}
                    aria-label={t("jumpToStep", { step: String(step) })}
                    title={t("jumpToStep", { step: String(step) })}
                  >
                    {line}
                  </button>
                  <button type="button" className="log-rerun-step" onClick={() => onRerunStep(step)}>
                    {t("rerunFailedStep")}
                  </button>
                </div>
              ) : (
                <button
                  className="log-line log-line-step"
                  key={`${index}-${line}`}
                  type="button"
                  onClick={() => onStepClick(step)}
                  aria-label={t("jumpToStep", { step: String(step) })}
                  title={t("jumpToStep", { step: String(step) })}
                >
                  {line}
                </button>
              )
            ) : (
              <div className="log-line" key={`${index}-${line}`}>
                {line || "\u00a0"}
              </div>
            )
          })}
          </div>
        </div>
      ) : tab === "variables" ? (
        <div id="execution-tabpanel-variables" role="tabpanel" aria-labelledby="execution-tab-variables" tabIndex={0}>
          {variables.length === 0 ? (
            <p className="panel-empty-guide">{t("variablesEmpty")}</p>
          ) : (
            <table className="variable-table">
              <thead>
                <tr>
                  <th scope="col">{t("variableName")}</th>
                  <th scope="col">{t("variableValue")}</th>
                </tr>
              </thead>
              <tbody>
                {variables.map((v, i) => (
                  <tr key={`${v.name}-${i}`}>
                    <td>{v.name}</td>
                    <td>{v.detail}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      ) : (
        <div id="execution-tabpanel-importedTable" role="tabpanel" aria-labelledby="execution-tab-importedTable" tabIndex={0}>
          <div className="execution-panel-toolbar">
            <button className="execution-panel-import-button" onClick={onImportTable}>
              {t("importTableButton")}
            </button>
          </div>
          {!importedTable ? (
            <p className="param-panel-empty">{t("noImportedData")}</p>
          ) : (
            <div className="imported-table-wrapper">
              <p className="imported-table-caption">{t("importedDataCaption", { name: importedTable.name })}</p>
              <table className="variable-table">
                <thead>
                  <tr>
                    <th scope="col" className="imported-table-checkbox-col">
                      <input
                        type="checkbox"
                        aria-label={t("selectAllRows")}
                        checked={importedTable.rows.length > 0 && checked.size === importedTable.rows.length}
                        onChange={(e) => {
                          const next = e.target.checked ? new Set(importedTable.rows.map((_, i) => i)) : new Set<number>()
                          setChecked(next)
                          onImportedRowsChange([...next].sort((a, b) => a - b))
                        }}
                      />
                    </th>
                    {importedTable.columns.map((col) => (
                      <th
                        key={col}
                        scope="col"
                        className="sortable-col-header"
                        aria-sort={sort?.col === col ? (sort.dir === "asc" ? "ascending" : "descending") : "none"}
                      >
                        <button
                          type="button"
                          onClick={() => toggleSort(col)}
                          aria-label={t("sortColumn", { column: col })}
                        >
                          {col}
                          {sort?.col === col ? (sort.dir === "asc" ? " ▲" : " ▼") : ""}
                        </button>
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {rowOrder.map((i) => {
                    const row = importedTable.rows[i]
                    return (
                      <tr key={i}>
                        <td className="imported-table-checkbox-col">
                          <input
                            type="checkbox"
                            aria-label={t("selectRow", { row: String(i + 1) })}
                            checked={checked.has(i)}
                            onChange={() => {
                              toggleRowChecked(i)
                              const next = new Set(checked)
                              if (next.has(i)) next.delete(i)
                              else next.add(i)
                              onImportedRowsChange([...next].sort((a, b) => a - b))
                            }}
                          />
                        </td>
                        {importedTable.columns.map((col) => (
                          <td key={col}>{row[col]}</td>
                        ))}
                      </tr>
                    )
                  })}
                </tbody>
              </table>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
