import { useEffect, useId, useRef, type ReactNode } from "react"
import { CloseIcon } from "./icons"

interface Props {
  title: ReactNode
  closeLabel: string
  onClose: () => void
  children: ReactNode
  className?: string
  busy?: boolean
}

export default function Modal({ title, closeLabel, onClose, children, className, busy = false }: Props) {
  const titleId = useId()
  const dialogRef = useRef<HTMLDivElement>(null)
  const restoreFocusRef = useRef<HTMLElement | null>(
    document.activeElement instanceof HTMLElement ? document.activeElement : null,
  )

  useEffect(() => {
    const element = restoreFocusRef.current
    return () => {
      if (element && document.contains(element)) element.focus()
    }
  }, [])

  useEffect(() => {
    const dialog = dialogRef.current
    if (!dialog) return
    const initialFocusable = dialog.querySelector<HTMLElement>(
      'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
    )
    if (!dialog.contains(document.activeElement)) (initialFocusable ?? dialog).focus()
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault()
        onClose()
        return
      }
      if (event.key !== "Tab") return
      const focusable = Array.from(
        dialog.querySelectorAll<HTMLElement>(
          'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
        ),
      )
      if (focusable.length === 0) {
        event.preventDefault()
        dialog.focus()
        return
      }
      const first = focusable[0]
      const last = focusable[focusable.length - 1]
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault()
        first.focus()
      }
    }
    dialog.addEventListener("keydown", handleKeyDown)
    return () => dialog.removeEventListener("keydown", handleKeyDown)
  }, [onClose])

  return (
    <div className="ai-suggest-overlay" onClick={onClose}>
      <div
        ref={dialogRef}
        className={`ai-suggest-modal${className ? ` ${className}` : ""}`}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-busy={busy}
        tabIndex={-1}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="ai-suggest-header">
          <span id={titleId}>{title}</span>
          <button type="button" className="chat-widget-close" onClick={onClose} title={closeLabel} aria-label={closeLabel}>
            <CloseIcon size={14} />
          </button>
        </div>
        <div className="ai-suggest-body">{children}</div>
      </div>
    </div>
  )
}
