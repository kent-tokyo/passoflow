import { useLocale } from "../i18n/useLocale"

interface Props {
  currentFile: string
  message: string
  version: string
  unsaved: boolean
}

export default function StatusBar({ currentFile, message, version, unsaved }: Props) {
  const { t } = useLocale()
  return (
    <div className="status-bar" role="status" aria-live="polite">
      <span className="status-bar-file">
        {currentFile || t("statusReady")}
        {unsaved && <span className="unsaved-status">{t("unsavedChanges")}</span>}
      </span>
      <span className="status-bar-right">
        <span className="status-bar-message">{message}</span>
        {version && <span className="status-bar-version">v{version}</span>}
      </span>
    </div>
  )
}
