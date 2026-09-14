/**
 * The pure half of lib/dispatch.ts: the `/__dispatch/latest` row shape and the card's one-line
 * reading of it. No alias imports and no React Query, so `node --test` can load it directly
 * (the same split grillForestChain.ts makes, for the same reason).
 */

export interface LatestRun {
  run_id: string
  status: string
  ended_by: string | null
  edits: number | null
  turns: number | null
  cost_usd: number | null
  end_time: string | null
  live: boolean
  bundle_path: string | null
}

/** The card's one-line reading of a delegated row. "in progress" only while a process is
 *  actually working on the row; a finished run says how it ended, e.g.
 *  `STUCK (no_progress_cutoff, 0 edits)`, so a row nobody is touching cannot pass as running. */
export function dispatchSummary(latest: LatestRun | undefined): string {
  if (!latest) return "delegated, no run recorded"
  if (latest.live) return "in progress"
  if (latest.status === "GREEN") return "GREEN, unlanded"
  // Rows logged before ended_by/edits existed have neither; the status alone still stands.
  const detail = [latest.ended_by, latest.edits === null ? null : `${latest.edits} edits`].filter(Boolean)
  return detail.length ? `${latest.status} (${detail.join(", ")})` : latest.status
}
