import { readStorage, writeStorage } from "./storage"

const METRICS_KEY = "passoflow-local-usage-metrics"

interface UsageMetrics {
  first_use_started_at?: number
  first_success_at?: number
  first_success_duration_ms?: number
  dom_setup_completed_at?: number
  last_run_started_at?: number
  last_failure_at?: number
  recovery_started_at?: number
  last_recovery_duration_ms?: number
  run_count?: number
  success_count?: number
}

function load(): UsageMetrics {
  const raw = readStorage(METRICS_KEY)
  if (!raw) return {}
  try {
    const parsed: unknown = JSON.parse(raw)
    return parsed && typeof parsed === "object" ? parsed as UsageMetrics : {}
  } catch {
    return {}
  }
}

function update(change: Partial<UsageMetrics>): void {
  writeStorage(METRICS_KEY, JSON.stringify({ ...load(), ...change }))
}

export function recordFirstUseStarted(): void {
  const metrics = load()
  if (metrics.first_use_started_at == null) update({ first_use_started_at: Date.now() })
}

export function recordDomSetupCompleted(): void {
  const metrics = load()
  if (metrics.dom_setup_completed_at == null) update({ dom_setup_completed_at: Date.now() })
}

export function recordRunStarted(): void {
  const metrics = load()
  update({ last_run_started_at: Date.now(), run_count: (metrics.run_count ?? 0) + 1 })
}

export function recordRunFinished(outcome: string): void {
  const now = Date.now()
  const metrics = load()
  const change: Partial<UsageMetrics> = {}
  if (outcome === "success") {
    change.success_count = (metrics.success_count ?? 0) + 1
    if (metrics.first_success_at == null) {
      change.first_success_at = now
      if (metrics.first_use_started_at != null) change.first_success_duration_ms = Math.max(0, now - metrics.first_use_started_at)
    }
    if (metrics.recovery_started_at != null) {
      change.last_recovery_duration_ms = Math.max(0, now - metrics.recovery_started_at)
      change.recovery_started_at = undefined
    }
  } else if (outcome === "failed") {
    change.last_failure_at = now
  }
  update(change)
}

export function recordRecoveryStarted(): void {
  update({ recovery_started_at: Date.now() })
}
