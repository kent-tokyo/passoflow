import { useEffect, useState } from "react"
import { readStorage, writeStorage } from "../lib/storage"

export const PALETTE_WIDTH = { min: 180, max: 420, default: 220 }
export const PARAMETER_PANEL_WIDTH = { min: 240, max: 480, default: 280 }

function loadWidth(key: string, fallback: number, min: number, max: number): number {
  const stored = Number(readStorage(key))
  return Number.isFinite(stored) ? Math.min(max, Math.max(min, stored)) : fallback
}

export function usePanelLayout() {
  const [paletteWidth, setPaletteWidth] = useState(() =>
    loadWidth("passoflow-action-palette-width", PALETTE_WIDTH.default, PALETTE_WIDTH.min, PALETTE_WIDTH.max),
  )
  const [parameterPanelWidth, setParameterPanelWidth] = useState(() =>
    loadWidth(
      "passoflow-parameter-panel-width",
      PARAMETER_PANEL_WIDTH.default,
      PARAMETER_PANEL_WIDTH.min,
      PARAMETER_PANEL_WIDTH.max,
    ),
  )

  useEffect(() => {
    writeStorage("passoflow-action-palette-width", String(paletteWidth))
  }, [paletteWidth])
  useEffect(() => {
    writeStorage("passoflow-parameter-panel-width", String(parameterPanelWidth))
  }, [parameterPanelWidth])

  return { paletteWidth, setPaletteWidth, parameterPanelWidth, setParameterPanelWidth }
}
