import { useState, type DragEvent } from "react"
import { useLocale } from "../i18n/useLocale"
import type { TranslationKey } from "../i18n/translations"
import type { ActionCategory, ActionSchema } from "../types/scenario"

interface Props {
  schemas: ActionSchema[]
  onAddAction: (action: string) => void
}

const CATEGORY_ORDER: ActionCategory[] = ["flow", "control", "app", "file", "excel", "screen", "variable", "input"]

const CATEGORY_KEYS: Record<ActionCategory, TranslationKey> = {
  flow: "categoryFlow",
  control: "categoryControl",
  app: "categoryApp",
  file: "categoryFile",
  excel: "categoryExcel",
  screen: "categoryScreen",
  variable: "categoryVariable",
  input: "categoryInput",
}

const CATEGORY_HELP_KEYS: Record<ActionCategory, TranslationKey> = {
  flow: "categoryHelpFlow",
  control: "categoryHelpControl",
  app: "categoryHelpApp",
  file: "categoryHelpFile",
  excel: "categoryHelpExcel",
  screen: "categoryHelpScreen",
  variable: "categoryHelpVariable",
  input: "categoryHelpInput",
}

// Keep the first view focused on the actions used in a typical GUI flow. The
// remaining groups stay one click away, and search always expands matches.
const DEFAULT_COLLAPSED_CATEGORIES = new Set<ActionCategory>(["app", "file", "excel", "variable"])

export default function ActionPalette({ schemas, onAddAction }: Props) {
  const { locale, t } = useLocale()
  const [search, setSearch] = useState("")
  const [collapsedCategories, setCollapsedCategories] = useState<Set<ActionCategory>>(DEFAULT_COLLAPSED_CATEGORIES)

  const onDragStart = (event: DragEvent, action: string) => {
    event.dataTransfer.setData("application/passoflow-action", action)
    event.dataTransfer.effectAllowed = "move"
  }

  const query = search.trim().toLowerCase()
  const visibleSchemas = query
    ? schemas.filter(
        (schema) =>
          schema.labels[locale].toLowerCase().includes(query) ||
          schema.purpose[locale].toLowerCase().includes(query) ||
          schema.action.toLowerCase().includes(query),
      )
    : schemas

  const groups = CATEGORY_ORDER.map((category) => ({
    category,
    schemas: visibleSchemas.filter((schema) => schema.category === category),
  })).filter((group) => group.schemas.length > 0)

  const toggleCategory = (category: ActionCategory) => {
    setCollapsedCategories((current) => {
      const next = new Set(current)
      if (next.has(category)) next.delete(category)
      else next.add(category)
      return next
    })
  }

  return (
    <aside className="palette" aria-labelledby="action-palette-title">
      <h3 id="action-palette-title">{t("actionsTitle")}</h3>
      <input
        type="text"
        className="palette-search"
        aria-label={t("searchActions")}
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        placeholder={t("searchActions")}
      />
      <p className="palette-help">{t("actionPaletteHelp")}</p>
      {groups.length === 0 && (
        <div className="palette-empty">
          <strong>{query ? t("actionSearchEmpty") : t("noItems")}</strong>
          {query && <p>{t("actionSearchEmptyDescription")}</p>}
        </div>
      )}
      <div className="palette-groups">
        {groups.map((group) => (
          <div key={group.category} className="palette-group">
            {(() => {
              const collapsed = collapsedCategories.has(group.category) && !query
              const contentId = `palette-group-${group.category}`
              return (
                <>
                  <h4 className="palette-group-title">
                    <button
                      type="button"
                      className="palette-group-toggle"
                      aria-expanded={!collapsed}
                      aria-controls={!collapsed ? contentId : undefined}
                      onClick={() => toggleCategory(group.category)}
                    >
                      <span aria-hidden="true">{collapsed ? "▸" : "▾"}</span>
                      {t(CATEGORY_KEYS[group.category])}
                    </button>
                  </h4>
                  {!collapsed && (
                    <div id={contentId}>
                      <p className="palette-group-help">{t(CATEGORY_HELP_KEYS[group.category])}</p>
                      {group.schemas.map((schema) => (
                        <button
                          key={schema.action}
                          className="palette-item"
                          type="button"
                          draggable
                          title={schema.action}
                          aria-label={`${t("addActionByClick")}: ${schema.labels[locale]}`}
                          onClick={() => onAddAction(schema.action)}
                          onDragStart={(e) => onDragStart(e, schema.action)}
                        >
                          <strong>{schema.labels[locale]}</strong>
                          <span className="palette-item-purpose">{schema.purpose[locale]}</span>
                        </button>
                      ))}
                    </div>
                  )}
                </>
              )
            })()}
          </div>
        ))}
      </div>
    </aside>
  )
}
