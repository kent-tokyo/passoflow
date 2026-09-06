import type { ReactNode } from "react"
import { ActionLabelContext } from "./ActionLabelContextBase"

export function ActionLabelProvider({ value, children }: { value: Record<string, string>; children: ReactNode }) {
  return <ActionLabelContext.Provider value={value}>{children}</ActionLabelContext.Provider>
}
