import { useEffect, useMemo, useState, type ReactNode } from "react"
import { LocaleContext, type LocaleContextValue } from "./LocaleContextBase"
import { translations, type Locale, type TranslationKey } from "./translations"
import { readStorage, writeStorage } from "../lib/storage"

const STORAGE_KEY = "passoflow-locale"
const DEFAULT_LOCALE: Locale = "en"

function readStoredLocale(): Locale {
  const stored = readStorage(STORAGE_KEY)
  return stored === "ja" || stored === "en" || stored === "zh" ? stored : DEFAULT_LOCALE
}

function interpolate(template: string, vars?: Record<string, string>): string {
  if (!vars) return template
  return template.replace(/\{(\w+)\}/g, (_match, key) => vars[key] ?? "")
}

export function LocaleProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(readStoredLocale)

  useEffect(() => {
    document.documentElement.lang = locale
  }, [locale])

  const value = useMemo<LocaleContextValue>(() => {
    const setLocale = (next: Locale) => {
      setLocaleState(next)
      writeStorage(STORAGE_KEY, next)
    }
    const t = (key: TranslationKey, vars?: Record<string, string>) => interpolate(translations[locale][key], vars)
    return { locale, setLocale, t }
  }, [locale])

  return <LocaleContext.Provider value={value}>{children}</LocaleContext.Provider>
}
