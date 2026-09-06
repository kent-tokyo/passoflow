import { lazy, Suspense } from "react"
import "./App.css"
import { useLocale } from "./i18n/useLocale"

const ScenarioEditor = lazy(() => import("./components/ScenarioEditor"))

function App() {
  const { t } = useLocale()
  return (
    <Suspense fallback={<div className="editor-loading" role="status" aria-live="polite">{t("loadingEditor")}</div>}>
      <ScenarioEditor />
    </Suspense>
  )
}

export default App
