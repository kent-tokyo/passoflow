import { Camera, Copy } from "lucide-react"
import { lazy, Suspense, useEffect, useRef, useState } from "react"
import { createPortal } from "react-dom"
import type { Node } from "reactflow"
import { fetchEnvironmentStatus, imageUrl, uploadScenarioImage } from "../api/scenarioApi"
import { useLocale } from "../i18n/useLocale"
import { KeyboardIcon } from "./icons"
import { captureChordFromEvent, captureKeyFromEvent } from "../lib/keyCapture"
import { useClickOutside } from "../lib/useClickOutside"
import type { ActionFieldSchema, ActionSchema, StepNodeData } from "../types/scenario"
const ScreenshotCropModal = lazy(() => import("./ScreenshotCropModal"))
const RegionPreviewModal = lazy(() => import("./RegionPreviewModal"))
const DomSelectorPreviewModal = lazy(() => import("./DomSelectorPreviewModal"))

interface Props {
  node: Node<StepNodeData> | null
  schemas: ActionSchema[]
  scenarioFilename: string
  scenarioFilenames: string[]
  variableNames: string[]
  onChange: (nodeId: string, params: Record<string, unknown>) => void
  onCommit: () => void
  onDelete: (nodeId: string) => void
}

function parseFieldValue(rawValue: string, type: string): unknown {
  if (type === "number") {
    const n = Number(rawValue)
    return Number.isFinite(n) ? n : undefined
  }
  if (type === "string[]" || type === "number[]") {
    const parts = rawValue
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean)
    return type === "number[]" ? parts.map(Number).filter(Number.isFinite) : parts
  }
  return rawValue
}

function fieldValueToText(value: unknown): string {
  if (value === undefined) return ""
  if (Array.isArray(value)) return value.join(", ")
  return String(value)
}

const FRIENDLY_FIELD_LABELS: Record<string, Record<"ja" | "en" | "zh", string>> = {
  path: { ja: "ファイルパス", en: "File path", zh: "文件路径" },
  destination: { ja: "移動先／コピー先", en: "Destination", zh: "目标位置" },
  new_name: { ja: "新しいファイル名", en: "New file name", zh: "新文件名" },
  if_destination_newer: { ja: "移動先が新しい場合のみ", en: "Only if destination is newer", zh: "仅当目标文件较新时" },
  drive: { ja: "割り当てるドライブ", en: "Drive letter", zh: "驱动器号" },
  name: { ja: "名前", en: "Name", zh: "名称" },
  args: { ja: "起動時の引数", en: "Launch arguments", zh: "启动参数" },
  text: { ja: "入力する文字列", en: "Text to enter", zh: "输入文本" },
  value: { ja: "値", en: "Value", zh: "值" },
  variable: { ja: "変数名", en: "Variable name", zh: "变量名" },
  title_contains: { ja: "ウィンドウタイトルに含まれる文字", en: "Window title contains", zh: "窗口标题包含" },
  url: { ja: "URL", en: "URL", zh: "URL" },
  selector: { ja: "CSSセレクタ", en: "CSS selector", zh: "CSS选择器" },
  timeout_ms: { ja: "タイムアウト（ミリ秒）", en: "Timeout (ms)", zh: "超时（毫秒）" },
  state: { ja: "待つ状態", en: "Wait state", zh: "等待状态" },
  retry: { ja: "再試行回数", en: "Retry count", zh: "重试次数" },
  retry_interval_ms: { ja: "再試行間隔（ミリ秒）", en: "Retry interval (ms)", zh: "重试间隔（毫秒）" },
  confidence: { ja: "画像一致度", en: "Image confidence", zh: "图像匹配度" },
  position: { ja: "クリック位置", en: "Click position", zh: "点击位置" },
  images: { ja: "検索画像", en: "Search images", zh: "搜索图像" },
  sheet: { ja: "シート名", en: "Sheet name", zh: "工作表名称" },
  cell: { ja: "セル", en: "Cell", zh: "单元格" },
  row: { ja: "行番号", en: "Row number", zh: "行号" },
  range: { ja: "対象範囲", en: "Target range", zh: "目标区域" },
  key_cell: { ja: "並べ替えの基準セル", en: "Sort key cell", zh: "排序关键单元格" },
  order: { ja: "並べ替え順", en: "Sort order", zh: "排序顺序" },
  macro: { ja: "マクロ名", en: "Macro name", zh: "宏名称" },
  method: { ja: "HTTPメソッド", en: "HTTP method", zh: "HTTP方法" },
  payload: { ja: "送信データ", en: "Request payload", zh: "请求数据" },
  click_type: { ja: "クリック方法", en: "Click type", zh: "点击方式" },
  offset: { ja: "クリック位置のずれ", en: "Click offset", zh: "点击偏移" },
  region: { ja: "検索範囲", en: "Search region", zh: "搜索区域" },
  region_origin: { ja: "検索範囲の基準", en: "Region origin", zh: "区域基准" },
  target_window_title: { ja: "対象ウィンドウ確認", en: "Target window check", zh: "目标窗口检查" },
  click_indicator_duration: { ja: "クリック表示時間（秒）", en: "Click indicator duration (s)", zh: "点击指示器时长（秒）" },
  ms: { ja: "待機時間（ミリ秒）", en: "Wait time (ms)", zh: "等待时间（毫秒）" },
  count: { ja: "繰り返し回数", en: "Repeat count", zh: "重复次数" },
  loop_table: { ja: "テーブル名", en: "Table name", zh: "表名称" },
  equals: { ja: "比較する値", en: "Value to compare", zh: "比较值" },
  days_offset: { ja: "日付のずらし日数", en: "Day offset", zh: "日期偏移天数" },
  months_offset: { ja: "月のずらし月数", en: "Month offset", zh: "月份偏移数" },
  key: { ja: "キー", en: "Key", zh: "按键" },
  keys: { ja: "キーの組み合わせ", en: "Key combination", zh: "按键组合" },
  wait: { ja: "キー入力後の待機時間（ミリ秒）", en: "Wait after key (ms)", zh: "按键后等待时间（毫秒）" },
}

function friendlyFieldLabel(name: string, locale: "ja" | "en" | "zh"): string {
  return FRIENDLY_FIELD_LABELS[name]?.[locale] ?? name.replaceAll("_", " ")
}

function NumberField({
  value,
  placeholder,
  ariaLabel,
  required = false,
  describedBy,
  onChangeText,
  onCommit,
}: {
  value: unknown
  placeholder: string
  ariaLabel: string
  required?: boolean
  describedBy?: string
  onChangeText: (raw: string) => void
  onCommit: () => void
}) {
  const [text, setText] = useState(() => fieldValueToText(value))
  const [focused, setFocused] = useState(false)

  // While focused, the field shows exactly what the user typed (including incomplete states
  // like "0." or "-") rather than the round-tripped Number->String of the parsed value, which
  // would silently eat characters like a trailing "." on every keystroke.
  useEffect(() => {
    if (!focused) setText(fieldValueToText(value))
  }, [value, focused])

  return (
    <input
      type="text"
      inputMode="decimal"
      aria-label={ariaLabel}
      required={required}
      aria-describedby={describedBy}
      value={text}
      placeholder={placeholder}
      onFocus={() => setFocused(true)}
      onChange={(e) => {
        setText(e.target.value)
        onChangeText(e.target.value)
      }}
      onBlur={() => {
        setFocused(false)
        onCommit()
      }}
    />
  )
}

function HintTooltip({ id, text }: { id: string; text: string }) {
  const iconRef = useRef<HTMLSpanElement>(null)
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null)

  const show = () => {
    const rect = iconRef.current?.getBoundingClientRect()
    if (!rect) return
    setPos({ top: rect.bottom + 6, left: Math.min(rect.left, window.innerWidth - 248) })
  }
  const hide = () => setPos(null)

  return (
    <>
      <span
        ref={iconRef}
        id={id}
        className="field-hint-icon"
        role="img"
        aria-label={text}
        onMouseEnter={show}
        onMouseLeave={hide}
        onFocus={show}
        onBlur={hide}
        tabIndex={0}
      >
        ?
      </span>
      {pos &&
        createPortal(
          <div className="field-hint-tooltip" role="tooltip" style={{ top: pos.top, left: pos.left }}>
            {text}
          </div>,
          document.body,
        )}
    </>
  )
}

function imagePathsForField(field: ActionFieldSchema, value: unknown): string[] {
  if (field.kind !== "image") return []
  if (field.type === "string[]") return Array.isArray(value) ? value.filter((v): v is string => Boolean(v)) : []
  return typeof value === "string" && value ? [value] : []
}

function ImageThumb({ path, missingLabel }: { path: string; missingLabel: string }) {
  const [missing, setMissing] = useState(false)
  if (missing) return <span className="image-preview-missing">{missingLabel}</span>
  return <img key={path} src={imageUrl(path)} alt={path} onError={() => setMissing(true)} />
}

function useKeyCapture(onCapture: (event: KeyboardEvent) => void) {
  const [listening, setListening] = useState(false)
  const onCaptureRef = useRef(onCapture)
  onCaptureRef.current = onCapture

  useEffect(() => {
    if (!listening) return
    const onKeyDown = (event: KeyboardEvent) => {
      event.preventDefault()
      event.stopPropagation()
      // A bare Escape (no modifiers held) cancels capture; Escape as part of a chord
      // (e.g. Ctrl+Shift+Esc) is recorded like any other key.
      const isPlainEscapeCancel = event.key === "Escape" && !event.ctrlKey && !event.shiftKey && !event.altKey && !event.metaKey
      if (!isPlainEscapeCancel) onCaptureRef.current(event)
      setListening(false)
    }
    window.addEventListener("keydown", onKeyDown, true)
    return () => window.removeEventListener("keydown", onKeyDown, true)
  }, [listening])

  return { listening, startListening: () => setListening(true) }
}

function KeySelect({
  value,
  options,
  onChange,
  onCommit,
  captureLabel,
  listeningLabel,
  ariaLabel,
  required = false,
  describedBy,
}: {
  value: string
  options: string[]
  onChange: (value: string) => void
  onCommit: () => void
  captureLabel: string
  listeningLabel: string
  ariaLabel: string
  required?: boolean
  describedBy?: string
}) {
  const { listening, startListening } = useKeyCapture((event) => {
    const key = captureKeyFromEvent(event)
    if (key) {
      onChange(key)
      onCommit()
    }
  })

  return (
    <div className="key-capture-row">
      <select
        aria-label={ariaLabel}
        required={required}
        aria-describedby={describedBy}
        value={value}
        onChange={(e) => {
          onChange(e.target.value)
          onCommit()
        }}
      >
        <option value="" disabled></option>
        {options.map((key) => (
          <option key={key} value={key}>
            {key}
          </option>
        ))}
      </select>
      <button
        type="button"
        className={`browse-button key-capture-button${listening ? " listening" : ""}`}
        title={captureLabel}
        onClick={startListening}
      >
        <KeyboardIcon size={14} />
        {listening ? listeningLabel : captureLabel}
      </button>
    </div>
  )
}

function KeyListEditor({
  value,
  options,
  onChange,
  addLabel,
  captureLabel,
  listeningLabel,
  removeLabel,
  ariaLabel,
  required = false,
  describedBy,
}: {
  value: string[]
  options: string[]
  onChange: (next: string[]) => void
  addLabel: string
  captureLabel: string
  listeningLabel: string
  removeLabel: (key: string) => string
  ariaLabel: string
  required?: boolean
  describedBy?: string
}) {
  const { listening, startListening } = useKeyCapture((event) => {
    const chord = captureChordFromEvent(event)
    if (chord.length > 0) onChange([...value, ...chord.filter((k) => !value.includes(k))])
  })

  return (
    <div className="key-list-editor">
      {value.length > 0 && (
        <div className="key-chip-list">
          {value.map((key, index) => (
            <span key={`${key}-${index}`} className="key-chip">
              {key}
              <button
                type="button"
                aria-label={removeLabel(key)}
                onClick={() => onChange(value.filter((_, i) => i !== index))}
              >
                ×
              </button>
            </span>
          ))}
        </div>
      )}
      <div className="key-capture-row">
        <select
          aria-label={ariaLabel}
          required={required}
          aria-describedby={describedBy}
          value=""
          onChange={(e) => {
            if (e.target.value) onChange([...value, e.target.value])
          }}
        >
          <option value="">{addLabel}</option>
          {options.map((key) => (
            <option key={key} value={key}>
              {key}
            </option>
          ))}
        </select>
        <button
          type="button"
          className={`browse-button key-capture-button${listening ? " listening" : ""}`}
          title={captureLabel}
          onClick={startListening}
        >
          <KeyboardIcon size={14} />
          {listening ? listeningLabel : captureLabel}
        </button>
      </div>
    </div>
  )
}

function ScenarioFilePicker({
  filenames,
  onSelect,
  browseLabel,
  fieldLabel,
  emptyLabel,
}: {
  filenames: string[]
  onSelect: (filename: string) => void
  browseLabel: string
  fieldLabel: string
  emptyLabel: string
}) {
  const [open, setOpen] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)
  const menuRef = useRef<HTMLDivElement>(null)

  useClickOutside(containerRef, open, () => setOpen(false))

  useEffect(() => {
    if (!open) return
    menuRef.current?.querySelector<HTMLButtonElement>("button:not([disabled])")?.focus()
  }, [open])

  return (
    <div className="dropdown" ref={containerRef}>
      <button
        type="button"
        className="browse-button"
        title={browseLabel}
        aria-label={`${fieldLabel}: ${browseLabel}`}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
      >
        {browseLabel}
      </button>
      {open && (
        <div
          ref={menuRef}
          className="dropdown-menu"
          role="menu"
          aria-label={browseLabel}
          onKeyDown={(event) => {
            const items = Array.from(menuRef.current?.querySelectorAll<HTMLButtonElement>("button:not([disabled])") ?? [])
            if (event.key === "Escape") {
              event.preventDefault()
              setOpen(false)
              return
            }
            if (items.length === 0 || !["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return
            event.preventDefault()
            const current = items.indexOf(document.activeElement as HTMLButtonElement)
            const next = event.key === "Home"
              ? 0
              : event.key === "End"
                ? items.length - 1
                : (current + (event.key === "ArrowUp" ? -1 : 1) + items.length) % items.length
            items[next]?.focus()
          }}
        >
          {filenames.length === 0 && <div className="context-menu-empty">{emptyLabel}</div>}
          {filenames.map((filename) => (
            <button
              key={filename}
              type="button"
              role="menuitem"
              className="dropdown-item"
              onClick={() => {
                onSelect(filename)
                setOpen(false)
              }}
            >
              {filename}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}

export default function ParameterPanel({
  node,
  schemas,
  scenarioFilename,
  scenarioFilenames,
  variableNames,
  onChange,
  onCommit,
  onDelete,
}: Props) {
  const { locale, t } = useLocale()
  const fileInputRefs = useRef<Record<string, HTMLInputElement | null>>({})
  const [uploadErrors, setUploadErrors] = useState<Record<string, boolean>>({})
  const [screenshotField, setScreenshotField] = useState<ActionFieldSchema | null>(null)
  const [selectorTest, setSelectorTest] = useState<{ value: string; valid: boolean } | null>(null)
  const [showRegionPreview, setShowRegionPreview] = useState(false)
  const [showDomSelectorPreview, setShowDomSelectorPreview] = useState(false)
  const isDomAction = node?.data.action.startsWith("browser_") ?? false
  const [domSetup, setDomSetup] = useState<"ready" | "playwright" | "chromium" | null>(null)
  const [setupCommandCopied, setSetupCommandCopied] = useState(false)
  const [copiedField, setCopiedField] = useState<string | null>(null)

  useEffect(() => {
    let active = true
    setDomSetup(null)
    setSetupCommandCopied(false)
    if (!isDomAction) return () => { active = false }
    fetchEnvironmentStatus().then(({ dom_browser }) => {
      if (!active) return
      setDomSetup(dom_browser.chromium ? "ready" : dom_browser.playwright ? "chromium" : "playwright")
    }).catch(() => {
      if (active) setDomSetup(null)
    })
    return () => { active = false }
  }, [isDomAction, node?.data.action])

  const setupCommand = domSetup === "playwright"
    ? "pip install playwright"
    : domSetup === "chromium"
      ? "python -m playwright install chromium"
      : null
  const copySetupCommand = () => {
    if (!setupCommand || !navigator.clipboard) return
    navigator.clipboard.writeText(setupCommand).then(() => {
      setSetupCommandCopied(true)
      window.setTimeout(() => setSetupCommandCopied(false), 1800)
    }).catch(() => setSetupCommandCopied(false))
  }

  const copyParameter = (field: ActionFieldSchema) => {
    const value = fieldValueToText(node?.data.params[field.name])
    if (!value || !navigator.clipboard) return
    navigator.clipboard.writeText(value).then(() => {
      setCopiedField(field.name)
      window.setTimeout(() => setCopiedField((current) => current === field.name ? null : current), 1800)
    }).catch(() => setCopiedField(null))
  }

  if (!node) {
    return (
      <aside className="param-panel" aria-label={t("parametersTitle")}>
        <p className="param-panel-empty">{t("selectNodePrompt")}</p>
        <ol className="param-panel-guide">
          <li>{t("editorGuideSelectAction")}</li>
          <li>{t("editorGuideConfigure")}</li>
          <li>{t("editorGuideRun")}</li>
        </ol>
      </aside>
    )
  }

  const schema = schemas.find((s) => s.action === node.data.action)
  const fields = schema?.fields ?? []

  const updateField = (name: string, rawValue: string, type: string) => {
    const params = { ...node.data.params }
    const parsed = rawValue === "" ? undefined : parseFieldValue(rawValue, type)
    if (parsed === undefined) {
      delete params[name]
    } else {
      params[name] = parsed
    }
    onChange(node.id, params)
  }

  const setFieldValue = (name: string, value: string | string[]) => {
    const params = { ...node.data.params }
    if (value.length === 0) {
      delete params[name]
    } else {
      params[name] = value
    }
    onChange(node.id, params)
  }

  const addImagePath = (field: ActionFieldSchema, path: string) => {
    const params = { ...node.data.params }
    if (field.type === "string[]") {
      const existing = Array.isArray(params[field.name]) ? (params[field.name] as string[]) : []
      params[field.name] = [...existing, path]
    } else {
      params[field.name] = path
    }
    onChange(node.id, params)
    onCommit()
  }

  const handleImageFile = async (field: ActionFieldSchema, fileList: FileList | null) => {
    const file = fileList?.[0]
    if (!file || !scenarioFilename) return
    setUploadErrors((prev) => ({ ...prev, [field.name]: false }))
    try {
      const { path } = await uploadScenarioImage(scenarioFilename, file)
      addImagePath(field, path)
    } catch {
      setUploadErrors((prev) => ({ ...prev, [field.name]: true }))
    }
  }

  const updateNote = (rawValue: string) => {
    const params = { ...node.data.params }
    if (rawValue === "") {
      delete params.note
    } else {
      params.note = rawValue
    }
    onChange(node.id, params)
  }

  const testSelector = (value: unknown) => {
    const selector = typeof value === "string" ? value.trim() : ""
    if (!selector) {
      setSelectorTest({ value: selector, valid: false })
      return
    }
    try {
      document.createElement("div").matches(selector)
      setSelectorTest({ value: selector, valid: true })
    } catch {
      setSelectorTest({ value: selector, valid: false })
    }
  }

  const updateTitle = (rawValue: string) => {
    const params = { ...node.data.params }
    if (rawValue === "") {
      delete params.title
    } else {
      params.title = rawValue
    }
    onChange(node.id, params)
  }

  return (
    <aside className="param-panel" aria-labelledby="parameter-panel-title">
      <h3 id="parameter-panel-title" title={node.data.action}>
        {schema?.labels[locale] ?? node.data.action}
        {node.data.flowStep != null && (
          <span className="param-panel-step">{t("selectedStepLabel", { step: String(node.data.flowStep) })}</span>
        )}
      </h3>
      {isDomAction && domSetup === "ready" && <p className="dom-setup-status ready">{t("domSetupReady")}</p>}
      {isDomAction && domSetup === "playwright" && (
        <p className="dom-setup-status warning">
          {t("domSetupPlaywrightMissing")}
          <button type="button" className="setup-command-copy" onClick={copySetupCommand}>
            {setupCommandCopied ? t("setupCommandCopied") : t("copySetupCommand")}
          </button>
        </p>
      )}
      {isDomAction && domSetup === "chromium" && (
        <p className="dom-setup-status warning">
          {t("domSetupChromiumMissing")}
          <button type="button" className="setup-command-copy" onClick={copySetupCommand}>
            {setupCommandCopied ? t("setupCommandCopied") : t("copySetupCommand")}
          </button>
        </p>
      )}
      <label className="param-field param-title">
        <span>{t("actionTitle")}</span>
        <input
          type="text"
          value={fieldValueToText(node.data.params.title)}
          onChange={(e) => updateTitle(e.target.value)}
          onBlur={onCommit}
          placeholder={t("actionTitlePlaceholder")}
        />
      </label>
      <label className="param-field param-note">
        <span>{t("note")}</span>
        <textarea
          value={fieldValueToText(node.data.params.note)}
          onChange={(e) => updateNote(e.target.value)}
          onBlur={onCommit}
          placeholder={t("notePlaceholder")}
        />
      </label>
      {fields.length === 0 && <p className="param-panel-empty">{t("noParams")}</p>}
      {fields.map((field) => {
        const imagePaths = imagePathsForField(field, node.data.params[field.name])
        const hintId = `parameter-hint-${field.name}`
        const describedBy = field.hint ? hintId : undefined
        return (
          <label key={field.name} className="param-field">
            <span>
              {friendlyFieldLabel(field.name, locale)}
              <code className="param-field-key">{field.name}</code>
              {field.required ? " *" : ""}
              {field.hint && <HintTooltip id={hintId} text={field.hint[locale]} />}
            </span>
            <div className="param-field-row">
              {field.kind === "key" && field.type === "string[]" ? (
                <KeyListEditor
                  value={Array.isArray(node.data.params[field.name]) ? (node.data.params[field.name] as string[]) : []}
                  options={field.options ?? []}
                  addLabel={t("addKey")}
                  captureLabel={t("captureKey")}
                  listeningLabel={t("listeningForKey")}
                  ariaLabel={friendlyFieldLabel(field.name, locale)}
                  required={field.required}
                  describedBy={describedBy}
                  removeLabel={(key) => t("removeKey", { key })}
                  onChange={(next) => {
                    setFieldValue(field.name, next)
                    onCommit()
                  }}
                />
              ) : field.kind === "key" ? (
                <KeySelect
                  value={typeof node.data.params[field.name] === "string" ? (node.data.params[field.name] as string) : ""}
                  options={field.options ?? []}
                  captureLabel={t("captureKey")}
                  listeningLabel={t("listeningForKey")}
                  ariaLabel={friendlyFieldLabel(field.name, locale)}
                  required={field.required}
                  describedBy={describedBy}
                  onChange={(value) => setFieldValue(field.name, value)}
                  onCommit={onCommit}
                />
              ) : field.kind === "select" ? (
                <select
                  aria-label={friendlyFieldLabel(field.name, locale)}
                  required={field.required}
                  aria-describedby={describedBy}
                  value={
                    typeof node.data.params[field.name] === "string"
                      ? (node.data.params[field.name] as string)
                      : field.default !== undefined
                        ? String(field.default)
                        : ""
                  }
                  onChange={(e) => {
                    updateField(field.name, e.target.value, field.type)
                    onCommit()
                  }}
                >
                  {(field.options ?? []).map((opt) => (
                    <option key={opt} value={opt}>
                      {opt}
                    </option>
                  ))}
                </select>
              ) : field.kind === "variable" ? (
                <>
                  <input
                    type="text"
                    aria-label={friendlyFieldLabel(field.name, locale)}
                    required={field.required}
                    aria-describedby={describedBy}
                    list={`variable-options-${field.name}`}
                    value={fieldValueToText(node.data.params[field.name])}
                    placeholder={field.default !== undefined ? String(field.default) : ""}
                    onChange={(e) => updateField(field.name, e.target.value, field.type)}
                    onBlur={onCommit}
                  />
                  <datalist id={`variable-options-${field.name}`}>
                    {variableNames.map((name) => (
                      <option key={name} value={name} />
                    ))}
                  </datalist>
                </>
              ) : field.type === "number" ? (
                <NumberField
                  value={node.data.params[field.name]}
                  placeholder={field.default !== undefined ? String(field.default) : ""}
                  ariaLabel={friendlyFieldLabel(field.name, locale)}
                  required={field.required}
                  describedBy={describedBy}
                  onChangeText={(raw) => updateField(field.name, raw, field.type)}
                  onCommit={onCommit}
                />
              ) : field.kind === "image" && field.type === "string[]" ? (
                <div className="image-candidate-editor" aria-label={t("imageCandidateList")}>
                  {imagePaths.map((path, index) => (
                    <div className="image-candidate-row" key={`${path}-${index}`}>
                      <span className="image-candidate-rank" aria-hidden="true">{index + 1}</span>
                      <input
                        type="text"
                        value={path}
                        aria-label={`${friendlyFieldLabel(field.name, locale)} ${index + 1}`}
                        onChange={(e) => {
                          const next = [...imagePaths]
                          next[index] = e.target.value
                          setFieldValue(field.name, next)
                        }}
                        onBlur={onCommit}
                      />
                      <button
                        type="button"
                        className="candidate-order-button"
                        disabled={index === 0}
                        aria-label={t("imageCandidateMoveUp", { index: String(index + 1) })}
                        onClick={() => {
                          const next = [...imagePaths]
                          ;[next[index - 1], next[index]] = [next[index], next[index - 1]]
                          setFieldValue(field.name, next)
                          onCommit()
                        }}
                      >↑</button>
                      <button
                        type="button"
                        className="candidate-order-button"
                        disabled={index === imagePaths.length - 1}
                        aria-label={t("imageCandidateMoveDown", { index: String(index + 1) })}
                        onClick={() => {
                          const next = [...imagePaths]
                          ;[next[index], next[index + 1]] = [next[index + 1], next[index]]
                          setFieldValue(field.name, next)
                          onCommit()
                        }}
                      >↓</button>
                      <button
                        type="button"
                        className="candidate-remove-button"
                        aria-label={t("imageCandidateRemove", { index: String(index + 1) })}
                        onClick={() => {
                          setFieldValue(field.name, imagePaths.filter((_, candidateIndex) => candidateIndex !== index))
                          onCommit()
                        }}
                      >×</button>
                    </div>
                  ))}
                  {imagePaths.length === 0 && <span className="image-candidate-empty">{t("noItems")}</span>}
                </div>
              ) : (
                <input
                  type="text"
                  aria-label={friendlyFieldLabel(field.name, locale)}
                  required={field.required}
                  aria-describedby={describedBy}
                  value={fieldValueToText(node.data.params[field.name])}
                  placeholder={field.default !== undefined ? String(field.default) : ""}
                  onChange={(e) => updateField(field.name, e.target.value, field.type)}
                  onBlur={onCommit}
                />
              )}
              {field.kind === "image" && (
                <>
                  <input
                    ref={(el) => {
                      fileInputRefs.current[field.name] = el
                    }}
                    type="file"
                    accept="image/png,image/jpeg,image/bmp"
                    className="visually-hidden"
                    onChange={(e) => {
                      void handleImageFile(field, e.target.files)
                      e.target.value = ""
                    }}
                  />
                  <button
                    type="button"
                    className="browse-button"
                    title={t("browse")}
                    aria-label={`${friendlyFieldLabel(field.name, locale)}: ${t("browse")}`}
                    onClick={() => fileInputRefs.current[field.name]?.click()}
                  >
                    {t("browse")}
                  </button>
                  <button
                    type="button"
                    className="browse-button"
                    title={t("captureScreenshot")}
                    aria-label={`${friendlyFieldLabel(field.name, locale)}: ${t("captureScreenshot")}`}
                    onClick={() => setScreenshotField(field)}
                  >
                    <Camera size={14} />
                  </button>
                </>
              )}
              {field.kind === "scenario" && (
                <ScenarioFilePicker
                  filenames={scenarioFilenames.filter((f) => f !== scenarioFilename)}
                  browseLabel={t("browse")}
                  fieldLabel={friendlyFieldLabel(field.name, locale)}
                  emptyLabel={t("noItems")}
                  onSelect={(filename) => {
                    updateField(field.name, filename, field.type)
                    onCommit()
                  }}
                />
              )}
              {field.name === "region" && (node.data.action === "click_image" || node.data.action === "move_mouse_to_image") && (
                <button type="button" className="browse-button" onClick={() => setShowRegionPreview(true)}>
                  {t("previewRegion")}
                </button>
              )}
              {field.name === "selector" && node.data.action.startsWith("browser_") && (
                <>
                  <button type="button" className="browse-button" onClick={() => testSelector(node.data.params.selector)}>
                    {t("testSelector")}
                  </button>
                  <button type="button" className="browse-button" onClick={() => setShowDomSelectorPreview(true)}>
                    {t("previewSelector")}
                  </button>
                </>
              )}
              {(field.type === "string" || field.type === "number") && !field.kind && fieldValueToText(node.data.params[field.name]) && (
                <button
                  type="button"
                  className="browse-button parameter-copy-button"
                  title={copiedField === field.name ? t("parameterCopied") : t("copyParameter")}
                  aria-label={`${friendlyFieldLabel(field.name, locale)}: ${copiedField === field.name ? t("parameterCopied") : t("copyParameter")}`}
                  onClick={() => copyParameter(field)}
                >
                  <Copy size={14} />
                </button>
              )}
            </div>
            {field.name === "selector" && node.data.action.startsWith("browser_") && selectorTest?.value === String(node.data.params.selector ?? "").trim() && (
              <span className={`selector-test-result ${selectorTest.valid ? "valid" : "invalid"}`} role="status">
                {selectorTest.valid ? t("selectorValid") : t("selectorInvalid")}
              </span>
            )}
            {copiedField === field.name && <span className="selector-test-result valid" role="status">{t("parameterCopied")}</span>}
            {uploadErrors[field.name] && <span className="param-field-error">{t("uploadImageFailed")}</span>}
            {imagePaths.length > 0 && (
              <div className="image-preview">
                {imagePaths.map((path) => (
                  <ImageThumb key={path} path={path} missingLabel={t("imageMissing", { path })} />
                ))}
              </div>
            )}
          </label>
        )
      })}
      <button className="danger-button" onClick={() => onDelete(node.id)}>
        {t("deleteNodeButton")}
      </button>
      {screenshotField && (
        <Suspense fallback={null}>
          <ScreenshotCropModal
            scenarioFilename={scenarioFilename}
            positionOptions={fields.find((field) => field.name === "position")?.options}
            initialPosition={typeof node.data.params.position === "string" ? node.data.params.position : "center"}
            onCaptured={(path, position) => {
              const params = { ...node.data.params }
              if (screenshotField.type === "string[]") {
                const existing = Array.isArray(params[screenshotField.name]) ? (params[screenshotField.name] as string[]) : []
                params[screenshotField.name] = [...existing, path]
              } else {
                params[screenshotField.name] = path
              }
              if (position) params.position = position
              onChange(node.id, params)
              onCommit()
            }}
            onClose={() => setScreenshotField(null)}
          />
        </Suspense>
      )}
      {showRegionPreview && (
        <Suspense fallback={null}>
          <RegionPreviewModal
            region={Array.isArray(node.data.params.region) ? node.data.params.region.filter((value): value is number => typeof value === "number") : null}
            regionOrigin={typeof node.data.params.region_origin === "string" ? node.data.params.region_origin : "screen"}
            onClose={() => setShowRegionPreview(false)}
          />
        </Suspense>
      )}
      {showDomSelectorPreview && (
        <Suspense fallback={null}>
          <DomSelectorPreviewModal
            initialUrl={typeof node.data.params.url === "string" ? node.data.params.url : ""}
            initialSelector={typeof node.data.params.selector === "string" ? node.data.params.selector : ""}
            onUseSelector={(selector) => {
              updateField("selector", selector, "string")
              onCommit()
              setShowDomSelectorPreview(false)
            }}
            onClose={() => setShowDomSelectorPreview(false)}
          />
        </Suspense>
      )}
    </aside>
  )
}
