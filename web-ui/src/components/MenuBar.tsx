import axios from "axios"
import { BookOpen, Check, FilePlus, Info } from "lucide-react"
import { useEffect, useRef, useState } from "react"
import { manualUrl, pickScenarioFile } from "../api/scenarioApi"
import { useLocale } from "../i18n/useLocale"
import { LOCALE_LABELS, LOCALES } from "../i18n/translations"
import { useClickOutside } from "../lib/useClickOutside"
import { useTheme } from "../lib/useTheme"
import { FolderIcon, PlayIcon, RedoIcon, SaveIcon, StopIcon, UndoIcon } from "./icons"

type MenuKey = "file" | "edit" | "view" | "help"
const MENU_KEYS: MenuKey[] = ["file", "edit", "view", "help"]

interface Props {
  onSelectScenario: (filename: string) => void
  onNewScenario: () => void
  onSave: () => void
  saveDisabled: boolean
  onSaveAs: () => void
  saveAsDisabled: boolean
  onRun: () => void
  onStop: () => void
  running: boolean
  runDisabled: boolean
  onUndo: () => void
  undoDisabled: boolean
  onRedo: () => void
  redoDisabled: boolean
  version: string
  focusMode: boolean
  onToggleFocusMode: () => void
}

export default function MenuBar({
  onSelectScenario,
  onNewScenario,
  onSave,
  saveDisabled,
  onSaveAs,
  saveAsDisabled,
  onRun,
  onStop,
  running,
  runDisabled,
  onUndo,
  undoDisabled,
  onRedo,
  redoDisabled,
  version,
  focusMode,
  onToggleFocusMode,
}: Props) {
  const { t, locale, setLocale } = useLocale()
  const { theme, toggleTheme } = useTheme()
  const [openMenu, setOpenMenu] = useState<MenuKey | null>(null)
  const [browsing, setBrowsing] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)
  const menuButtonRefs = useRef<Record<MenuKey, HTMLButtonElement | null>>({
    file: null,
    edit: null,
    view: null,
    help: null,
  })

  const handleOpenScenario = async () => {
    setBrowsing(true)
    try {
      const picked = await pickScenarioFile()
      if (picked) onSelectScenario(picked)
    } catch (e) {
      const detail = axios.isAxiosError(e) ? (e.response?.data as { detail?: unknown } | undefined)?.detail : undefined
      window.alert(typeof detail === "string" ? detail : t("openFileDialogFailed"))
    } finally {
      setBrowsing(false)
    }
  }

  useClickOutside(containerRef, openMenu !== null, () => setOpenMenu(null))

  useEffect(() => {
    if (!openMenu) return
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpenMenu(null)
    }
    window.addEventListener("keydown", onKeyDown)
    return () => window.removeEventListener("keydown", onKeyDown)
  }, [openMenu])

  const toggleMenu = (key: MenuKey) => setOpenMenu((current) => (current === key ? null : key))
  // Classic menu-bar feel: once a menu is open, hovering a different top-level item switches to it directly.
  const hoverMenu = (key: MenuKey) => {
    if (openMenu && openMenu !== key) setOpenMenu(key)
  }

  const focusFirstMenuItem = (key: MenuKey) => {
    window.setTimeout(() => {
      const menu = containerRef.current?.querySelector(`[data-menu="${key}"]`)
      menu?.querySelector<HTMLButtonElement>("button:not([disabled])")?.focus()
    }, 0)
  }

  const handleMenuButtonKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>, key: MenuKey) => {
    if (["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) {
      event.preventDefault()
      const current = MENU_KEYS.indexOf(key)
      const next = event.key === "Home"
        ? 0
        : event.key === "End"
          ? MENU_KEYS.length - 1
          : (current + (event.key === "ArrowLeft" ? -1 : 1) + MENU_KEYS.length) % MENU_KEYS.length
      const nextKey = MENU_KEYS[next]
      menuButtonRefs.current[nextKey]?.focus()
      if (openMenu) setOpenMenu(nextKey)
      return
    }
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return
    event.preventDefault()
    setOpenMenu(key)
    focusFirstMenuItem(key)
  }

  const handleDropdownKeyDown = (event: React.KeyboardEvent<HTMLDivElement>, key: MenuKey) => {
    const items = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>("button:not([disabled])"))
    if (event.key === "Escape") {
      event.preventDefault()
      setOpenMenu(null)
      menuButtonRefs.current[key]?.focus()
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
  }

  return (
    <div
      className="menu-bar"
      ref={containerRef}
      role="menubar"
      aria-label={t("menuBar")}
      onMouseLeave={() => setOpenMenu(null)}
    >
      <div className="menu-dropdown">
        <button
          type="button"
          ref={(element) => { menuButtonRefs.current.file = element }}
          className="menu-bar-item"
          role="menuitem"
          aria-haspopup="menu"
          aria-expanded={openMenu === "file"}
          aria-controls={openMenu === "file" ? "menu-panel-file" : undefined}
          onClick={() => toggleMenu("file")}
          onKeyDown={(event) => handleMenuButtonKeyDown(event, "file")}
          onMouseEnter={() => hoverMenu("file")}
        >
          {t("menuFile")}
        </button>
        {openMenu === "file" && (
          <div id="menu-panel-file" className="dropdown-menu menu-dropdown-panel" role="menu" aria-label={t("menuFile")} data-menu="file" onKeyDown={(event) => handleDropdownKeyDown(event, "file")}>
            <button
              className="dropdown-item"
              role="menuitem"
              onClick={() => {
                onNewScenario()
                setOpenMenu(null)
              }}
            >
              <span className="menu-item-label">
                <FilePlus size={14} />
                {t("newScenarioMenu")}
              </span>
            </button>
            <button
              className="dropdown-item"
              role="menuitem"
              onClick={() => {
                void handleOpenScenario()
                setOpenMenu(null)
              }}
              disabled={browsing}
            >
              <span className="menu-item-label">
                <FolderIcon />
                {t("browseScenarios")}
              </span>
            </button>
            <button
              className="dropdown-item menu-item-with-shortcut"
              role="menuitem"
              onClick={() => {
                onSave()
                setOpenMenu(null)
              }}
              disabled={saveDisabled}
            >
              <span className="menu-item-label">
                <SaveIcon />
                {t("save")}
              </span>
              <span className="menu-shortcut">Ctrl+S</span>
            </button>
            <button
              className="dropdown-item"
              role="menuitem"
              onClick={() => {
                onSaveAs()
                setOpenMenu(null)
              }}
              disabled={saveAsDisabled}
            >
              <span className="menu-item-label">
                <SaveIcon />
                {t("saveAs")}
              </span>
            </button>
            <button
              className="dropdown-item"
              role="menuitem"
              onClick={() => {
                if (running) onStop()
                else onRun()
                setOpenMenu(null)
              }}
              disabled={runDisabled}
            >
              <span className="menu-item-label">
                {running ? <StopIcon /> : <PlayIcon />}
                {running ? t("stop") : t("run")}
              </span>
            </button>
          </div>
        )}
      </div>
      <div className="menu-dropdown">
        <button
          type="button"
          ref={(element) => { menuButtonRefs.current.edit = element }}
          className="menu-bar-item"
          role="menuitem"
          aria-haspopup="menu"
          aria-expanded={openMenu === "edit"}
          aria-controls={openMenu === "edit" ? "menu-panel-edit" : undefined}
          onClick={() => toggleMenu("edit")}
          onKeyDown={(event) => handleMenuButtonKeyDown(event, "edit")}
          onMouseEnter={() => hoverMenu("edit")}
        >
          {t("menuEdit")}
        </button>
        {openMenu === "edit" && (
          <div id="menu-panel-edit" className="dropdown-menu menu-dropdown-panel" role="menu" aria-label={t("menuEdit")} data-menu="edit" onKeyDown={(event) => handleDropdownKeyDown(event, "edit")}>
            <button
              className="dropdown-item menu-item-with-shortcut"
              role="menuitem"
              onClick={() => {
                onUndo()
                setOpenMenu(null)
              }}
              disabled={undoDisabled}
            >
              <span className="menu-item-label">
                <UndoIcon />
                {t("undo")}
              </span>
              <span className="menu-shortcut">Ctrl+Z</span>
            </button>
            <button
              className="dropdown-item menu-item-with-shortcut"
              role="menuitem"
              onClick={() => {
                onRedo()
                setOpenMenu(null)
              }}
              disabled={redoDisabled}
            >
              <span className="menu-item-label">
                <RedoIcon />
                {t("redo")}
              </span>
              <span className="menu-shortcut">Ctrl+Y</span>
            </button>
          </div>
        )}
      </div>
      <div className="menu-dropdown">
        <button
          type="button"
          ref={(element) => { menuButtonRefs.current.view = element }}
          className="menu-bar-item"
          role="menuitem"
          aria-haspopup="menu"
          aria-expanded={openMenu === "view"}
          aria-controls={openMenu === "view" ? "menu-panel-view" : undefined}
          onClick={() => toggleMenu("view")}
          onKeyDown={(event) => handleMenuButtonKeyDown(event, "view")}
          onMouseEnter={() => hoverMenu("view")}
        >
          {t("menuView")}
        </button>
        {openMenu === "view" && (
          <div id="menu-panel-view" className="dropdown-menu menu-dropdown-panel" role="menu" aria-label={t("menuView")} data-menu="view" onKeyDown={(event) => handleDropdownKeyDown(event, "view")}>
            <button role="menuitem" className="dropdown-item menu-item-with-shortcut" onClick={toggleTheme}>
              <span className="menu-item-label">
                <span className="menu-item-check">{theme === "dark" && <Check size={14} />}</span>
                {t("darkMode")}
              </span>
            </button>
            <button
              role="menuitemcheckbox"
              aria-checked={focusMode}
              className="dropdown-item menu-item-with-shortcut"
              onClick={onToggleFocusMode}
            >
              <span className="menu-item-label">
                <span className="menu-item-check">{focusMode && <Check size={14} />}</span>
                {t("focusCanvasMode")}
              </span>
              <span className="menu-shortcut">Ctrl+Shift+F</span>
            </button>
            <div className="menu-dropdown-divider" />
            {LOCALES.map((l) => (
              <button key={l} role="menuitem" className="dropdown-item menu-item-with-shortcut" onClick={() => setLocale(l)}>
                <span className="menu-item-label">
                  <span className="menu-item-check">{l === locale && <Check size={14} />}</span>
                  {LOCALE_LABELS[l]}
                </span>
              </button>
            ))}
          </div>
        )}
      </div>
      <div className="menu-dropdown">
        <button
          type="button"
          ref={(element) => { menuButtonRefs.current.help = element }}
          className="menu-bar-item"
          role="menuitem"
          aria-haspopup="menu"
          aria-expanded={openMenu === "help"}
          aria-controls={openMenu === "help" ? "menu-panel-help" : undefined}
          onClick={() => toggleMenu("help")}
          onKeyDown={(event) => handleMenuButtonKeyDown(event, "help")}
          onMouseEnter={() => hoverMenu("help")}
        >
          {t("menuHelp")}
        </button>
        {openMenu === "help" && (
          <div id="menu-panel-help" className="dropdown-menu menu-dropdown-panel" role="menu" aria-label={t("menuHelp")} data-menu="help" onKeyDown={(event) => handleDropdownKeyDown(event, "help")}>
            <button
              className="dropdown-item"
              role="menuitem"
              onClick={() => {
                window.open(manualUrl(locale), "_blank")
                setOpenMenu(null)
              }}
            >
              <span className="menu-item-label">
                <BookOpen size={14} />
                {t("manual")}
              </span>
            </button>
            <button
              className="dropdown-item"
              role="menuitem"
              onClick={() => {
                window.alert(t("versionMessage", { version }))
                setOpenMenu(null)
              }}
            >
              <span className="menu-item-label">
                <Info size={14} />
                {t("version")}
              </span>
            </button>
          </div>
        )}
      </div>
    </div>
  )
}
