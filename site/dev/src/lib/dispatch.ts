/**
 * The dispatch pipeline's read side for the board (vite-plugins/dispatch.ts): the latest run per
 * row from plans/dispatch-log.csv joined with what is live right now, and one run's bundle. A
 * `delegated` row in board.yaml says only that a run was started; whether anything is still
 * running, or how it ended, lives here.
 */

import { useQuery } from "@tanstack/react-query"

import type { LatestRun } from "./dispatchSummary.ts"

export { dispatchSummary, type LatestRun } from "./dispatchSummary.ts"

export type LatestRuns = Record<string, LatestRun>

export interface Attempt {
  n: number
  ending: string
  moved: boolean
  turns: number
  verify_tail: string
}

export interface SelfReport {
  blocker_kind: string
  narrower_would_succeed: boolean
  explanation: string
}

/** status.json plus the three text files the run endpoint reads alongside it. Every field is
 *  optional on the wire: a bundle can be missing any of its files, and the endpoint returns
 *  nulls rather than failing. */
export interface RunBundle {
  run_id?: string
  row_id?: string
  model?: string
  start_time?: string
  end_time?: string
  duration_s?: number
  status?: string
  cost_usd?: number
  patch_path?: string | null
  bundle_path?: string | null
  ended_by?: string | null
  edits?: number | null
  turns?: number | null
  attempts?: Attempt[]
  self_report?: SelfReport | null
  self_report_text: string | null
  brief: string | null
  agent_log_tail: string | null
}

/** A run takes minutes and ends once, so a slow poll is enough to catch the live -> finished
 *  transition; the 1.2s cadence lib/grillForest.ts uses is for a typing-speed conversation. */
const POLL_INTERVAL_MS = 10_000

async function json<T>(res: Response): Promise<T> {
  if (!res.ok) throw new Error(await res.text())
  return res.json() as Promise<T>
}

/** One query shared by every card on the page: React Query dedups by key, so thirty delegated
 *  cards mounting together cost one request, not thirty. */
export function useDispatchLatest() {
  return useQuery({
    queryKey: ["dispatch", "latest"],
    queryFn: () => fetch("/__dispatch/latest").then((r) => json<LatestRuns>(r)),
    refetchInterval: POLL_INTERVAL_MS,
  })
}

export function useDispatchRun(rowId: string, runId: string) {
  return useQuery({
    queryKey: ["dispatch", "run", rowId, runId],
    queryFn: () => fetch(`/__dispatch/run/${rowId}/${runId}`).then((r) => json<RunBundle>(r)),
  })
}

export function runHref(rowId: string, runId: string): string {
  return `#/run/${rowId}/${runId}`
}

export function agentLogHref(rowId: string, runId: string): string {
  return `/__dispatch/run/${rowId}/${runId}/agent.log`
}
