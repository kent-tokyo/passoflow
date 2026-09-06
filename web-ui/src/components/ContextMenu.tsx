import { useEffect, useRef } from "react"
import { useLocale } from "../i18n/useLocale"

export interface ContextMenuItem {
  label: string
  onClick: () => void
  danger?: boolean
}

export interface ContextMenuSection {
  title?: string
  items: ContextMenuItem[]
}

interface Props {
  x: number
  y: number
  sections: ContextMenuSection[]
  onClose: () => void
}

export default function ContextMenu({ x, y, sections, onClose }: Props) {
  const { t } = useLocale()
  const menuRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const menu = menuRef.current
    if (!menu) return
    const items = () => Array.from(menu.querySelectorAll<HTMLButtonElement>("button:not([disabled])"))
    items()[0]?.focus()
    const handleKeyDown = (event: KeyboardEvent) => {
      const menuItems = items()
      if (event.key === "Escape") {
        event.preventDefault()
        onClose()
        return
      }
      if (menuItems.length === 0 || !["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return
      event.preventDefault()
      const current = menuItems.indexOf(document.activeElement as HTMLButtonElement)
      const next = event.key === "Home"
        ? 0
        : event.key === "End"
          ? menuItems.length - 1
          : (current + (event.key === "ArrowUp" ? -1 : 1) + menuItems.length) % menuItems.length
      menuItems[next]?.focus()
    }
    menu.addEventListener("keydown", handleKeyDown)
    return () => menu.removeEventListener("keydown", handleKeyDown)
  }, [onClose])

  return (
    <div
      className="context-menu-overlay"
      onClick={onClose}
      onContextMenu={(e) => {
        e.preventDefault()
        onClose()
      }}
    >
      <div
        ref={menuRef}
        className="context-menu"
        role="menu"
        aria-label={t("contextMenu")}
        style={{ left: x, top: y }}
        onClick={(e) => e.stopPropagation()}
      >
        {sections.map((section, sectionIndex) => (
          <div key={sectionIndex} className="context-menu-section">
            {section.title && <div className="context-menu-title">{section.title}</div>}
            {section.items.length === 0 && <div className="context-menu-empty">{t("noItems")}</div>}
            {section.items.map((item, itemIndex) => (
              <button
                key={itemIndex}
                type="button"
                role="menuitem"
                className={`context-menu-item${item.danger ? " danger" : ""}`}
                onClick={() => {
                  item.onClick()
                  onClose()
                }}
              >
                {item.label}
              </button>
            ))}
          </div>
        ))}
      </div>
    </div>
  )
}
