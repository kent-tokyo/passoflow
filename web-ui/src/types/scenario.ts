export interface ActionFieldSchema {
  name: string
  type: "string" | "number" | "string[]" | "number[]"
  required: boolean
  default?: unknown
  kind?: "image" | "scenario" | "key" | "select" | "variable"
  options?: string[]
  hint?: Record<"ja" | "en" | "zh", string>
}

export type ActionCategory = "flow" | "control" | "app" | "file" | "excel" | "screen" | "variable" | "input"

export interface ActionSchema {
  action: string
  category: ActionCategory
  labels: Record<"ja" | "en" | "zh", string>
  purpose: Record<"ja" | "en" | "zh", string>
  fields: ActionFieldSchema[]
  outcomes?: {
    success: boolean
    warning_continue: boolean
    failure_stop: boolean
  }
}

export type ScenarioStep = { action: string } & Record<string, unknown>

export interface ScenarioSummary {
  filename: string
  title: string
}

export interface ScenarioData {
  title: string
  steps: ScenarioStep[]
}

export interface StepNodeData {
  action: string
  params: Record<string, unknown>
  group?: string
  loop?: string
  loopCount?: number
  loopTable?: string
  flowStep?: number
  onRequestContextMenu?: (x: number, y: number) => void
  onNudgeNode?: (id: string, dx: number, dy: number) => void
  onFocusAdjacentNode?: (id: string, direction: "left" | "right" | "up" | "down") => void
}
