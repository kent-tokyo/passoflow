import axios from "axios"
import type { ActionSchema, ScenarioData, ScenarioStep, ScenarioSummary } from "../types/scenario"

const API_BASE = "http://127.0.0.1:8000"

/** Encode each path segment individually so "/" keeps working as a folder separator. */
function encodePath(relPath: string): string {
  return relPath.split("/").map(encodeURIComponent).join("/")
}

export async function fetchVersion(): Promise<string> {
  const res = await axios.get<{ version: string }>(`${API_BASE}/api/version`)
  return res.data.version
}

export function manualUrl(lang: string): string {
  return `${API_BASE}/docs/manual?lang=${encodeURIComponent(lang)}`
}

export async function fetchActionSchemas(): Promise<ActionSchema[]> {
  const res = await axios.get<ActionSchema[]>(`${API_BASE}/api/actions`)
  return res.data
}

export interface EnvironmentStatus {
  dom_browser: {
    playwright: boolean
    chromium: boolean
    message: string
  }
}

export async function fetchEnvironmentStatus(): Promise<EnvironmentStatus> {
  const res = await axios.get<EnvironmentStatus>(`${API_BASE}/api/environment`)
  return res.data
}

export interface DomPreviewResult {
  url: string
  selector: string
  count: number
  samples: Array<{ tag: string; text: string; id: string; testid: string; visible: boolean; selector: string }>
  suggested_selector: string | null
  repair_suggestions: Array<{ selector: string; count: number }>
}

export async function previewDomSelector(url: string, selector: string): Promise<DomPreviewResult> {
  const res = await axios.post<DomPreviewResult>(`${API_BASE}/api/dom/preview`, { url, selector })
  return res.data
}

export async function fetchScenarioList(): Promise<ScenarioSummary[]> {
  const res = await axios.get<ScenarioSummary[]>(`${API_BASE}/api/scenarios`)
  return res.data
}

export async function fetchScenario(filename: string): Promise<ScenarioData> {
  const res = await axios.get<ScenarioData>(`${API_BASE}/api/scenarios/${encodePath(filename)}`)
  return res.data
}

export async function saveScenario(filename: string, title: string, steps: ScenarioStep[]): Promise<void> {
  await axios.put(`${API_BASE}/api/scenarios/${encodePath(filename)}`, { title, steps })
}

export async function createScenario(filename: string, title: string, steps: ScenarioStep[]): Promise<void> {
  await axios.post(`${API_BASE}/api/scenarios`, { filename, title, steps })
}

export async function deleteScenario(filename: string): Promise<void> {
  await axios.delete(`${API_BASE}/api/scenarios/${encodePath(filename)}`)
}

export async function moveScenario(filename: string, destination: string): Promise<void> {
  await axios.post(`${API_BASE}/api/scenarios/${encodePath(filename)}/move`, { destination })
}

/** Download a scenario (plus every call_scenario/repeat target and image it references) as a zip. */
export async function exportScenario(filename: string): Promise<void> {
  const res = await axios.get(`${API_BASE}/api/scenarios/${encodePath(filename)}/export`, { responseType: "blob" })
  const disposition = res.headers["content-disposition"] as string | undefined
  const match = disposition?.match(/filename\*=UTF-8''([^;]+)/)
  const fallbackName = `${(filename.split("/").pop() ?? filename).replace(/\.ya?ml$/i, "")}.zip`
  const downloadName = match ? decodeURIComponent(match[1]) : fallbackName

  const url = URL.createObjectURL(res.data as Blob)
  const a = document.createElement("a")
  a.href = url
  a.download = downloadName
  document.body.appendChild(a)
  a.click()
  a.remove()
  URL.revokeObjectURL(url)
}

/** Extract a scenario package (as produced by exportScenario) back into scenarios/. */
export async function importScenarioZip(file: File): Promise<{ scenarios: string[] }> {
  const formData = new FormData()
  formData.append("file", file)
  const res = await axios.post<{ scenarios: string[] }>(`${API_BASE}/api/scenarios/import`, formData)
  return res.data
}

export async function fetchFolders(): Promise<string[]> {
  const res = await axios.get<string[]>(`${API_BASE}/api/folders`)
  return res.data
}

export async function createFolder(path: string): Promise<void> {
  await axios.post(`${API_BASE}/api/folders`, { path })
}

export async function deleteFolder(path: string): Promise<void> {
  await axios.delete(`${API_BASE}/api/folders/${encodePath(path)}`)
}

export async function moveFolder(path: string, destination: string): Promise<void> {
  await axios.post(`${API_BASE}/api/folders/${encodePath(path)}/move`, { destination })
}

export async function startRun(
  filename: string,
  range?: { start?: number; end?: number },
): Promise<{ run_id: string }> {
  const res = await axios.post<{ run_id: string; filename: string }>(
    `${API_BASE}/api/scenarios/${encodePath(filename)}/run`,
    null,
    { params: range },
  )
  return res.data
}

export async function validateScenario(filename: string): Promise<{ errors: string[]; warnings: string[] }> {
  const res = await axios.get<{ errors: string[]; warnings: string[] }>(
    `${API_BASE}/api/scenarios/${encodePath(filename)}/validate`,
  )
  return res.data
}

export function openRunSocket(runId: string, filename: string): WebSocket {
  return new WebSocket(`ws://127.0.0.1:8000/ws/runs/${runId}?filename=${encodeURIComponent(filename)}`)
}

export async function stopRun(runId: string): Promise<void> {
  await axios.post(`${API_BASE}/api/runs/${runId}/stop`)
}

export async function uploadScenarioImage(filename: string, file: File): Promise<{ path: string }> {
  const formData = new FormData()
  formData.append("file", file)
  const res = await axios.post<{ path: string }>(
    `${API_BASE}/api/scenarios/${encodePath(filename)}/images`,
    formData,
  )
  return res.data
}

export function imageUrl(relPath: string): string {
  return `${API_BASE}/api/scenario-images/${encodePath(relPath)}`
}

export async function captureScreenshot(): Promise<Blob> {
  const res = await axios.get<Blob>(`${API_BASE}/api/screenshot`, { responseType: "blob" })
  return res.data
}

export interface ChatMessage {
  role: "user" | "assistant"
  content: string
}

export interface FlowAction {
  action: string
  params: Record<string, unknown>
}

export interface SelectedStep {
  action: string
  params: Record<string, unknown>
}

export interface ChatResponse {
  reply: string
  actions: FlowAction[]
}

export async function sendChatMessage(messages: ChatMessage[], selectedStep?: SelectedStep): Promise<ChatResponse> {
  const res = await axios.post<ChatResponse>(`${API_BASE}/api/chat`, {
    messages,
    ...(selectedStep ? { selected_action: selectedStep.action, selected_params: selectedStep.params } : {}),
  })
  return res.data
}

export interface ActionSuggestion {
  action: string
  params: Record<string, unknown>
  explanation: string
}

export async function suggestAction(
  action: string,
  params: Record<string, unknown>,
  instruction: string,
): Promise<ActionSuggestion> {
  const res = await axios.post<ActionSuggestion>(`${API_BASE}/api/actions/suggest`, { action, params, instruction })
  return res.data
}

export interface TableImportResult {
  columns: string[]
  rows: Record<string, string>[]
}

/** Read an .xlsx/.csv file's column names and rows, for the editor's "取り込み" import flow. */
export async function importTable(path: string, sheet: string): Promise<TableImportResult> {
  const res = await axios.post<TableImportResult>(`${API_BASE}/api/tables/import`, {
    path,
    sheet: sheet.trim() || null,
  })
  return res.data
}

/** Opens a native "Open File" dialog on the machine running the backend (this is a local
 *  desktop app, so that's always the user's own machine) and returns the chosen absolute
 *  path, or null if cancelled. */
export async function pickTableFile(): Promise<string | null> {
  const res = await axios.post<{ path: string | null }>(`${API_BASE}/api/tables/pick-file`)
  return res.data.path
}

/** Opens a native "Open File" dialog rooted at the scenarios folder, filtered to .yaml/.yml,
 *  and returns the chosen file as a scenarios-relative path (ready for onSelect/fetchScenario),
 *  or null if cancelled. */
export async function pickScenarioFile(): Promise<string | null> {
  const res = await axios.post<{ path: string | null }>(`${API_BASE}/api/scenarios/pick-file`)
  return res.data.path
}

/** Opens a native "Save As" dialog rooted at the scenarios folder, filtered to .yaml/.yml
 *  (the OS itself confirms overwriting an existing file), and returns the chosen destination
 *  as a scenarios-relative path, or null if cancelled. */
export async function pickScenarioSaveFile(defaultName: string): Promise<string | null> {
  const res = await axios.post<{ path: string | null }>(`${API_BASE}/api/scenarios/pick-save-file`, {
    default_name: defaultName,
  })
  return res.data.path
}
