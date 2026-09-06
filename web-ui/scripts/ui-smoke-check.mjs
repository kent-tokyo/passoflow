import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

const root = join(dirname(fileURLToPath(import.meta.url)), "..")

async function source(relativePath) {
  return readFile(join(root, relativePath), "utf8")
}

async function checkCanvasRendering() {
  const editor = await source("src/components/ScenarioEditor.tsx")
  const canvas = await source("src/components/FlowCanvas.tsx")

  assert.match(editor, /import FlowCanvas from ["']\.\/FlowCanvas["']/)
  assert.doesNotMatch(editor, /<ReactFlow\b/)
  assert.match(canvas, /<ReactFlow\b/)
  assert.match(canvas, /onMouseMove=\{onMouseMove\}/)
}

async function checkResizablePanelHandle() {
  const panelHandle = await source("src/components/PanelResizeHandle.tsx")

  assert.match(panelHandle, /role=["']separator["']/)
  assert.match(panelHandle, /onKeyDown/)
  assert.match(panelHandle, /ArrowLeft|ArrowRight/)
}

async function checkHeaderMenus() {
  const menuBar = await source("src/components/MenuBar.tsx")

  assert.match(menuBar, /onMouseLeave/)
  assert.match(menuBar, /setOpenMenu\(null\)/)
}

async function checkManualLinks() {
  const server = await source("../src/api_server.py")

  assert.match(server, /actions\.md.*docs\/manual\?lang=en/)
  assert.match(server, /actions_ja\.md.*docs\/manual\?lang=ja/)
  assert.match(server, /extensions=\["tables",\s*"fenced_code"\]/)
  assert.match(server, /pre code\[class\*="language-"\]/)
  assert.match(server, /\.manual-code-label/)
  assert.match(server, /heading_number/)
  assert.match(server, /f"\{heading_number\} \{title\}"/)
}

async function checkManualAudience() {
  const server = await source("../src/api_server.py")
  const guideJa = await source("../docs/guide_ja.md")
  const guideEn = await source("../docs/guide.md")

  assert.match(server, /audience: str = "user"/)
  assert.match(server, /guide_ja\.md/)
  assert.match(server, /guide\.md/)
  assert.match(server, /@app\.get\("\/docs\/assets\/\{filename\}"\)/)
  assert.match(server, /screenshot_passoflow_01\.png/)
  assert.match(guideJa, /^# 一般ユーザー向け/m)
  assert.match(guideEn, /^# Using PassoFlow/m)
  assert.match(guideJa, /docs-assets\/screenshot_passoflow_01\.png/)
  assert.match(guideEn, /docs-assets\/screenshot_passoflow_01\.png/)
  assert.match(guideJa, /audience=advanced/)
  assert.match(guideEn, /audience=advanced/)
}

async function checkSafeStorageAndRunGuard() {
  const storage = await source("src/lib/storage.ts")
  const editor = await source("src/components/ScenarioEditor.tsx")
  const execution = await source("src/hooks/useScenarioExecution.ts")

  assert.match(storage, /try \{/) 
  assert.match(storage, /catch \{/) 
  assert.match(editor, /useScenarioExecution/)
  assert.match(execution, /runStartingRef/)
  assert.match(execution, /const isCurrentRun = \(\) =>/)
}

async function checkHistoryLimit() {
  const history = await source("src/hooks/useFlowHistory.ts")

  assert.match(history, /MAX_FLOW_HISTORY = 100/)
  assert.match(history, /slice\(-MAX_FLOW_HISTORY\)/)
}

async function checkResizeCleanup() {
  const resizeHook = await source("src/hooks/useWindowDragResize.ts")
  const panelHandle = await source("src/components/PanelResizeHandle.tsx")
  const executionPanel = await source("src/components/ExecutionPanel.tsx")

  assert.match(resizeHook, /window\.removeEventListener\("mousemove"/)
  assert.match(resizeHook, /useEffect\(\(\) => \(\) => cleanupRef\.current\?\.\(\), \[\]\)/)
  assert.match(panelHandle, /useWindowDragResize/)
  assert.match(executionPanel, /useWindowDragResize/)
}

await checkCanvasRendering()
await checkResizablePanelHandle()
await checkHeaderMenus()
await checkManualLinks()
await checkManualAudience()
await checkSafeStorageAndRunGuard()
await checkHistoryLimit()
await checkResizeCleanup()
console.log("UI smoke checks passed (7 checks)")
