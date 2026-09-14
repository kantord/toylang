import assert from "node:assert/strict"
import { test } from "node:test"

import { dispatchSummary, type LatestRun } from "./dispatchSummary.ts"

const finished: LatestRun = {
  run_id: "36d59b9d",
  status: "STUCK",
  ended_by: "no_progress_cutoff",
  edits: 0,
  turns: 12,
  cost_usd: 0.16,
  end_time: "2026-09-13T16:55:02+00:00",
  live: false,
  bundle_path: null,
}

// The incident this guards against: twelve rows sat `delegated` in board.yaml for days after
// their runs ended STUCK, and the card read every one as "in progress".
test("a finished run is never 'in progress', however board.yaml labels the row", () => {
  assert.equal(dispatchSummary(finished), "STUCK (no_progress_cutoff, 0 edits)")
})

test("only a live process reads as in progress", () => {
  assert.equal(dispatchSummary({ ...finished, live: true }), "in progress")
})

test("a green run that has not landed says so", () => {
  assert.equal(dispatchSummary({ ...finished, status: "GREEN", ended_by: "model_done", edits: 3 }), "GREEN, unlanded")
})

test("rows logged before ended_by/edits existed fall back to the bare status", () => {
  assert.equal(dispatchSummary({ ...finished, ended_by: null, edits: null }), "STUCK")
})

test("a delegated row with no CSV row at all is called out", () => {
  assert.equal(dispatchSummary(undefined), "delegated, no run recorded")
})
