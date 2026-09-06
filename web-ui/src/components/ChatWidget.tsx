import { useEffect, useRef, useState } from "react"
import { sendChatMessage, type ChatMessage, type FlowAction, type SelectedStep } from "../api/scenarioApi"
import { useLocale } from "../i18n/useLocale"
import { ChatIcon, CloseIcon, SendIcon } from "./icons"

interface ChatWidgetProps {
  selectedStep?: SelectedStep | null
  onInsertActions?: (actions: FlowAction[]) => void
}

export default function ChatWidget({ selectedStep, onInsertActions }: ChatWidgetProps) {
  const { t } = useLocale()
  const [open, setOpen] = useState(false)
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [input, setInput] = useState("")
  const [sending, setSending] = useState(false)
  const [error, setError] = useState("")
  const historyRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLTextAreaElement>(null)
  const toggleRef = useRef<HTMLButtonElement>(null)
  const wasOpenRef = useRef(false)

  useEffect(() => {
    if (historyRef.current) historyRef.current.scrollTop = historyRef.current.scrollHeight
  }, [messages, sending])

  useEffect(() => {
    if (open) {
      inputRef.current?.focus()
    } else if (wasOpenRef.current) {
      toggleRef.current?.focus()
    }
    wasOpenRef.current = open
  }, [open])

  const handleSend = async () => {
    const text = input.trim()
    if (!text || sending) return
    const next = [...messages, { role: "user", content: text } as ChatMessage]
    setMessages(next)
    setInput("")
    setError("")
    setSending(true)
    try {
      const response = await sendChatMessage(next, selectedStep ?? undefined)
      const inserted = response.actions.length > 0
      setMessages((prev) => [
        ...prev,
        {
          role: "assistant",
          content: inserted
            ? response.reply + "\n\n" + t("chatActionsInserted", { count: String(response.actions.length) })
            : response.reply,
        },
      ])
      if (inserted) onInsertActions?.(response.actions)
    } catch {
      setError(t("chatFailed"))
    } finally {
      setSending(false)
    }
  }

  if (!open) {
    return (
      <button
        type="button"
        ref={toggleRef}
        className="chat-toggle"
        aria-label={t("chatTitle")}
        aria-expanded={false}
        onClick={() => setOpen(true)}
        title={t("chatTitle")}
      >
        <ChatIcon />
      </button>
    )
  }

  return (
    <aside className="chat-widget" aria-label={t("chatTitle")}>
      <div className="chat-widget-header">
        <span>{t("chatTitle")}</span>
        <button type="button" className="chat-widget-close" aria-label={t("close")} onClick={() => setOpen(false)} title={t("close")}>
          <CloseIcon size={14} />
        </button>
      </div>
      <div className="chat-widget-history" ref={historyRef} role="log" aria-live="polite" aria-relevant="additions text">
        {messages.length === 0 && <p className="chat-widget-empty">{t("chatEmpty")}</p>}
        {messages.map((m, i) => (
          <div key={i} className={`chat-message chat-message-${m.role}`}>
            {m.content}
          </div>
        ))}
        {sending && <div className="chat-message chat-message-assistant chat-message-pending">{t("chatThinking")}</div>}
        {error && <div className="chat-widget-error" role="alert">{error}</div>}
      </div>
      <div className="chat-widget-input-row">
        <textarea
          ref={inputRef}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder={t("chatPlaceholder")}
          aria-label={t("chatPlaceholder")}
          onKeyDown={(event) => {
            if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
              event.preventDefault()
              void handleSend()
            }
          }}
          rows={2}
        />
        <button type="button" aria-label={t("chatSend")} onClick={() => void handleSend()} disabled={sending || !input.trim()} title={t("chatSend")}>
          <SendIcon />
        </button>
      </div>
    </aside>
  )
}
