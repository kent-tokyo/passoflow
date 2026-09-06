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

async function checkFailureArtifactRecovery() {
  const executionPanel = await source("src/components/ExecutionPanel.tsx")
  const editor = await source("src/components/ScenarioEditor.tsx")
  const translations = await source("src/i18n/translations.ts")

  assert.match(executionPanel, /Failure screenshots saved/)
  assert.match(executionPanel, /onRerunStep\(step\)/)
  assert.match(editor, /rerunStepWithCountdown/)
  assert.match(editor, /setTimeout\(\(\) =>/)
  assert.match(translations, /rerunFailedStep:/)
}

async function checkImageCandidateGuidance() {
  const server = await source("../src/api_server.py")
  const screenActions = await source("../src/screen_actions.py")

  assert.match(server, /click_image\.images/)
  assert.match(server, /move_mouse_to_image\.images/)
  assert.match(server, /候補画像は上から順に試します/)
  assert.match(screenActions, /Matched image candidate/)
}

async function checkDomSetupGuidance() {
  const server = await source("../src/api_server.py")
  const webActions = await source("../src/web_actions.py")
  const guide = await source("src/components/FirstUseGuide.tsx")
  const api = await source("src/api/scenarioApi.ts")
  const panel = await source("src/components/ParameterPanel.tsx")

  assert.match(server, /@app\.get\("\/api\/environment"\)/)
  assert.match(webActions, /dom_browser_setup_status/)
  assert.match(guide, /firstUseGuideChromiumMissing/)
  assert.match(api, /fetchEnvironmentStatus/)
  assert.match(panel, /domSetupChromiumMissing/)
  assert.match(panel, /copySetupCommand/)
  assert.match(panel, /navigator\.clipboard\.writeText/)
}

async function checkWindowRelativeImageRegion() {
  const server = await source("../src/api_server.py")
  const runner = await source("../src/run_scenario.py")
  const screenActions = await source("../src/screen_actions.py")
  const manual = await source("../docs/actions.md")

  assert.match(server, /region_origin/)
  assert.match(runner, /REGION_ORIGINS/)
  assert.match(screenActions, /resolve_search_region/)
  assert.match(screenActions, /getActiveWindow/)
  assert.match(screenActions, /ensure_target_window/)
  assert.match(runner, /target_window_title/)
  assert.match(server, /target_window_title/)
  assert.match(manual, /active_window/)
}

async function checkRegionPreview() {
  const panel = await source("src/components/ParameterPanel.tsx")
  const preview = await source("src/components/RegionPreviewModal.tsx")
  const translations = await source("src/i18n/translations.ts")

  assert.match(panel, /RegionPreviewModal/)
  assert.match(panel, /previewRegion/)
  assert.match(preview, /captureScreenshot/)
  assert.match(preview, /regionPreviewActiveWindowHint/)
  assert.match(translations, /regionPreviewTitle:/)
}

async function checkOutcomeContract() {
  const runner = await source("../src/run_scenario.py")
  const server = await source("../src/api_server.py")
  const tests = await source("../tests/test_run_scenario_outcomes.py")

  assert.match(runner, /action_outcome_contract/)
  assert.match(runner, /_WARNING_CONTINUE_ACTIONS/)
  assert.match(server, /"outcomes": action_outcome_contract/)
  assert.match(tests, /test_action_outcome_contract_marks_recoverable_warnings/)
}

async function checkSelectorSyntaxCheck() {
  const panel = await source("src/components/ParameterPanel.tsx")
  const translations = await source("src/i18n/translations.ts")

  assert.match(panel, /testSelector/)
  assert.match(panel, /\.matches\(selector\)/)
  assert.match(panel, /selector-test-result/)
  assert.match(translations, /selectorInvalid:/)
}

async function checkDomSelectorPreview() {
  const server = await source("../src/api_server.py")
  const webActions = await source("../src/web_actions.py")
  const panel = await source("src/components/ParameterPanel.tsx")
  const modal = await source("src/components/DomSelectorPreviewModal.tsx")
  const api = await source("src/api/scenarioApi.ts")

  assert.match(server, /@app\.post\("\/api\/dom\/preview"\)/)
  assert.match(webActions, /preview_dom_selector/)
  assert.match(webActions, /headless=True/)
  assert.match(panel, /DomSelectorPreviewModal/)
  assert.match(modal, /domSelectorPreviewMatches/)
  assert.match(modal, /onUseSelector/)
  assert.match(modal, /domSelectorCapture/)
  assert.match(modal, /domSelectorRecoveryTimeout/)
  assert.match(modal, /repair_suggestions/)
  assert.match(webActions, /repair_suggestions/)
  assert.match(webActions, /selector: element\.id/)
  assert.match(api, /previewDomSelector/)
}

async function checkLocalUsageMetrics() {
  const metrics = await source("src/lib/usageMetrics.ts")
  const execution = await source("src/hooks/useScenarioExecution.ts")
  const guide = await source("src/components/FirstUseGuide.tsx")
  const editor = await source("src/components/ScenarioEditor.tsx")

  assert.match(metrics, /passoflow-local-usage-metrics/)
  assert.match(metrics, /first_success_duration_ms/)
  assert.match(metrics, /last_recovery_duration_ms/)
  assert.match(execution, /recordRunStarted\(\)/)
  assert.match(execution, /recordRunFinished/)
  assert.match(guide, /recordFirstUseStarted/)
  assert.match(guide, /recordDomSetupCompleted/)
  assert.match(editor, /recordRecoveryStarted/)
}

async function checkParameterCopy() {
  const panel = await source("src/components/ParameterPanel.tsx")
  const translations = await source("src/i18n/translations.ts")

  assert.match(panel, /copyParameter/)
  assert.match(panel, /navigator\.clipboard\.writeText\(value\)/)
  assert.match(panel, /parameter-copy-button/)
  assert.match(translations, /parameterCopied:/)
}

await checkCanvasRendering()
await checkResizablePanelHandle()
await checkHeaderMenus()
await checkManualLinks()
await checkManualAudience()
await checkSafeStorageAndRunGuard()
await checkHistoryLimit()
await checkResizeCleanup()
await checkFailureArtifactRecovery()
await checkImageCandidateGuidance()
await checkDomSetupGuidance()
await checkWindowRelativeImageRegion()
await checkRegionPreview()
await checkOutcomeContract()
await checkSelectorSyntaxCheck()
await checkDomSelectorPreview()
await checkLocalUsageMetrics()
await checkParameterCopy()
console.log("UI smoke checks passed (17 checks)")
