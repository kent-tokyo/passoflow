import { createContext } from "react"
import type { Locale, TranslationKey } from "./translations"

export interface LocaleContextValue {
  locale: Locale
  setLocale: (locale: Locale) => void
  t: (key: TranslationKey, vars?: Record<string, string>) => string
}

export const LocaleContext = createContext<LocaleContextValue | null>(null)
