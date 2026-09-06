import type { ReactNode } from "react"
import { RunningNodeContext } from "./RunningNodeContextBase"

export function RunningNodeProvider({ value, children }: { value: string | null; children: ReactNode }) {
  return <RunningNodeContext.Provider value={value}>{children}</RunningNodeContext.Provider>
}
