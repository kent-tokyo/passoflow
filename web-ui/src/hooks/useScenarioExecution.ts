import axios from "axios"
import { useCallback, useEffect, useRef, type Dispatch, type SetStateAction } from "react"
import type { Edge, Node } from "reactflow"
import { openRunSocket, startRun, stopRun, validateScenario } from "../api/scenarioApi"
import { orderNodes } from "../lib/flowConversion"
import { writeStorage } from "../lib/storage"
import { recordRunFinished, recordRunStarted } from "../lib/usageMetrics"
import type { TranslationKey } from "../i18n/translations"
import type { StepNodeData } from "../types/scenario"
import type { RunContext } from "../components/ExecutionPanel"

interface Props {
  currentFile: string
  running: boolean
  nodes: Node<StepNodeData>[]
  edges: Edge[]
  t: (key: TranslationKey, vars?: Record<string, string>) => string
  setLogs: Dispatch<SetStateAction<string[]>>
  setRunning: Dispatch<SetStateAction<boolean>>
  setRunningStep: Dispatch<SetStateAction<number | null>>
  setStatusMessage: Dispatch<SetStateAction<string>>
  setRunContext: Dispatch<SetStateAction<RunContext | null>>
}

export function useScenarioExecution({
  currentFile,
  running,
  nodes,
  edges,
  t,
  setLogs,
  setRunning,
  setRunningStep,
  setStatusMessage,
  setRunContext,
}: Props) {
  const currentRunIdRef = useRef<string | null>(null)
  const runSocketRef = useRef<WebSocket | null>(null)
  const runStartingRef = useRef(false)
  const lastCompletedStepRef = useRef<number | null>(null)
  const activeRunContextRef = useRef<Omit<RunContext, "outcome" | "lastCompletedStep"> | null>(null)

  useEffect(() => () => {
    runSocketRef.current?.close()
    runSocketRef.current = null
    currentRunIdRef.current = null
    runStartingRef.current = false
  }, [])

  const handleRun = useCallback(async (range?: { start?: number; end?: number }) => {
    if (!currentFile || running || runStartingRef.current) return
    runStartingRef.current = true
    runSocketRef.current?.close()
    const resetStarting = () => { runStartingRef.current = false }
    setStatusMessage(t("preparingRun"))
    const preflightContext = [t("preflightTargetFilename", { filename: currentFile }), t("preflightFilenameFormat")]
    setLogs([t("preflightHeader"), ...preflightContext])

    let preflight: { errors: string[]; warnings: string[] }
    try {
      preflight = await validateScenario(currentFile)
    } catch (error: unknown) {
      const detail = axios.isAxiosError(error)
        ? typeof error.response?.data?.detail === "string" ? error.response.data.detail : error.message
        : error instanceof Error ? error.message : String(error)
      const failureMessage = t("preflightFailedDetail", { detail })
      setLogs([t("preflightHeader"), ...preflightContext, t("preflightFailed"), failureMessage])
      setStatusMessage(failureMessage)
      resetStarting()
      return
    }

    const totalSteps = orderNodes(nodes, edges).length
    const hasSelectedScope = range?.start != null || range?.end != null
    const scopeLine = hasSelectedScope
      ? t("preflightSelectedScope", { start: String(range?.start ?? 1), end: String(range?.end ?? totalSteps) })
      : t("preflightFullScope")
    const preflightLines = [t("preflightHeader"), ...preflightContext, scopeLine]
    if (preflight.errors.length > 0) preflightLines.push(t("preflightErrors"), ...preflight.errors.map((error) => `- ${error}`))
    if (preflight.warnings.length > 0) preflightLines.push(t("preflightWarnings"), ...preflight.warnings.map((warning) => `- ${warning}`))
    if (preflight.errors.length === 0 && preflight.warnings.length === 0) preflightLines.push(t("preflightPassed"))
    setLogs(preflightLines)
    if (preflight.errors.length > 0) {
      setStatusMessage(t("preflightBlocked"))
      resetStarting()
      return
    }

    try {
      setRunning(true)
      recordRunStarted()
      setStatusMessage(t("runningScenario"))
      setRunningStep(null)
      lastCompletedStepRef.current = null
      activeRunContextRef.current = { filename: currentFile, startedAt: new Date().toISOString(), start: range?.start, end: range?.end }
      const { run_id } = await startRun(currentFile, range)
      currentRunIdRef.current = run_id
      const socket = openRunSocket(run_id, currentFile)
      runSocketRef.current = socket
      const isCurrentRun = () => currentRunIdRef.current === run_id && runSocketRef.current === socket
      socket.onmessage = (event) => {
        if (!isCurrentRun()) return
        let message: { type: string; [key: string]: unknown }
        try { message = JSON.parse(event.data) } catch { return }
        if (message.type === "log") setLogs((prev) => [...prev, message.line as string])
        else if (message.type === "progress") setRunningStep(message.step as number)
        else if (message.type === "completed") lastCompletedStepRef.current = message.step as number
        else if (message.type === "feedback") setLogs((prev) => [...prev, "", t("runFeedbackHeader"), message.text as string])
        else if (message.type === "done") {
          const completionMessage = lastCompletedStepRef.current == null ? t("runNoCompletedStep") : t("runLastCompleted", { step: String(lastCompletedStepRef.current) })
          const outcomeMessages: Record<string, string> = { success: t("runOutcomeSuccess"), warning: t("runOutcomeWarning"), stopped: t("runOutcomeStopped"), failed: t("runOutcomeFailed") }
          const outcomeMessage = outcomeMessages[String(message.outcome)] ?? t("runFinished", { code: String(message.returncode) })
          recordRunFinished(String(message.outcome ?? "unknown"))
          const nextRunContext: RunContext = { ...(activeRunContextRef.current ?? { filename: currentFile, startedAt: new Date().toISOString() }), outcome: String(message.outcome ?? "unknown"), lastCompletedStep: lastCompletedStepRef.current }
          setStatusMessage(outcomeMessage)
          setRunContext(nextRunContext)
          writeStorage("passoflow-last-run-context", JSON.stringify(nextRunContext))
          setLogs((prev) => [...prev, completionMessage, outcomeMessage, t("runFinished", { code: String(message.returncode) })])
          currentRunIdRef.current = null
          runSocketRef.current = null
          runStartingRef.current = false
          setRunning(false)
          setRunningStep(null)
        }
      }
      socket.onerror = () => {
        if (!isCurrentRun()) return
        setLogs((prev) => [...prev, t("runConnectionError")])
        currentRunIdRef.current = null; runSocketRef.current = null; runStartingRef.current = false
        setRunning(false); setRunningStep(null)
      }
      socket.onclose = () => {
        if (!isCurrentRun()) return
        currentRunIdRef.current = null; runSocketRef.current = null; runStartingRef.current = false
        setRunning(false); setRunningStep(null)
      }
    } catch {
      setStatusMessage(t("runStartFailed"))
      currentRunIdRef.current = null; runStartingRef.current = false
      setRunning(false); setRunningStep(null)
    }
  }, [currentFile, edges, nodes, running, setLogs, setRunContext, setRunning, setRunningStep, setStatusMessage, t])

  const handleStop = useCallback(async () => {
    const runId = currentRunIdRef.current
    if (!runId) return
    try { setStatusMessage(t("stoppingScenario")); await stopRun(runId) }
    catch { setStatusMessage(t("stopRunFailed")) }
  }, [setStatusMessage, t])

  return { handleRun, handleStop }
}
