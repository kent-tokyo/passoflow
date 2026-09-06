import { useEffect, useState } from "react"
import { readStorage, writeStorage } from "./storage"

const STORAGE_KEY = "passoflow-theme"

export type Theme = "light" | "dark"

function readStoredTheme(): Theme | null {
  const stored = readStorage(STORAGE_KEY)
  return stored === "light" || stored === "dark" ? stored : null
}

function getInitialTheme(): Theme {
  return readStoredTheme() ?? (window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light")
}

export function useTheme() {
  const [theme, setThemeState] = useState<Theme>(getInitialTheme)

  useEffect(() => {
    document.documentElement.dataset.theme = theme
  }, [theme])

  const setTheme = (next: Theme) => {
    setThemeState(next)
    writeStorage(STORAGE_KEY, next)
  }

  const toggleTheme = () => setTheme(theme === "dark" ? "light" : "dark")

  return { theme, toggleTheme }
}
