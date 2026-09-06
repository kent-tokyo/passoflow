import { useContext } from "react"
import { ActionLabelContext } from "./ActionLabelContextBase"

export function useActionLabel(action: string): string {
  const labels = useContext(ActionLabelContext)
  return labels[action] ?? action
}
