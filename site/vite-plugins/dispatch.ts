import { execFile } from "node:child_process"
import { readFile } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import { promisify } from "node:util"

import type { Plugin } from "vite"

/**
 * Dev-only read side of the dispatch pipeline: the latest run per board row from
 * plans/dispatch-log.csv, joined with which rows a simple_dispatch.py process is working on right
 * now, plus one run's bundle directory. This exists because the board card used to render every
 * `status: delegated` row as "in progress" from board.yaml alone, which hid twelve rows that had
 * all finished STUCK days earlier with nothing running (plans/dispatch-self-healing-plan.md).
 */

const REPO = path.resolve(import.meta.dirname, "..", "..")
const CSV = path.join(REPO, "plans", "dispatch-log.csv")
const RESULTS = path.join(os.homedir(), ".cache", "toylang-simple-dispatch", "results")
const LOG_TAIL_LINES = 200

/** Both ids are used as path segments under RESULTS, so nothing outside this set gets near the
 *  filesystem. Row ids are board slugs and run ids are hex, so this is not restrictive. */
const SAFE_ID = /^[A-Za-z0-9_-]+$/

interface LatestRun {
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

const run = promisify(execFile)

async function liveRowIds(): Promise<Set<string>> {
  const { stdout } = await run(
    "uv",
    ["run", "--project", ".claude/scripts", ".claude/scripts/dispatch_state.py", "--live"],
    { cwd: REPO },
  )
  return new Set(stdout.split("\n").filter(Boolean))
}

function numberOrNull(s: string | undefined): number | null {
  return s === undefined || s === "" ? null : Number(s)
}

/** Latest CSV row per row_id, keyed by header name so a column appended later (the last four
 *  were) reads as empty on older rows instead of shifting everything. Fields are split on bare
 *  commas: the writer only ever emits ids, ISO timestamps, numbers and absolute paths, none of
 *  which contain one. */
async function latestRuns(): Promise<Map<string, Record<string, string>>> {
  let text: string
  try {
    text = await readFile(CSV, "utf8")
  } catch {
    return new Map()
  }
  const [header, ...lines] = text.split("\n").filter(Boolean)
  const cols = header.split(",")
  const latest = new Map<string, Record<string, string>>()
  for (const line of lines) {
    const cells = line.split(",")
    const row: Record<string, string> = {}
    cols.forEach((c, i) => (row[c] = cells[i] ?? ""))
    const prev = latest.get(row.row_id)
    if (!prev || Date.parse(row.start_time) > Date.parse(prev.start_time)) latest.set(row.row_id, row)
  }
  return latest
}

async function readText(file: string): Promise<string | null> {
  try {
    return await readFile(file, "utf8")
  } catch {
    return null
  }
}

async function readJson(file: string): Promise<unknown> {
  const text = await readText(file)
  if (text === null) return null
  try {
    return JSON.parse(text)
  } catch {
    return null
  }
}

function tail(text: string | null, n: number): string | null {
  if (text === null) return null
  // The file's trailing newline would otherwise count as a 201st, empty, line.
  const lines = text.replace(/\n$/, "").split("\n")
  return lines.slice(Math.max(0, lines.length - n)).join("\n")
}

function sendJson(res: import("node:http").ServerResponse, body: unknown) {
  res.statusCode = 200
  res.setHeader("Content-Type", "application/json")
  res.end(JSON.stringify(body))
}

export function dispatchLog(): Plugin {
  return {
    name: "dispatch-log",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use("/__dispatch/latest", async (req, res) => {
        if (req.method !== "GET") {
          res.statusCode = 405
          res.end()
          return
        }
        let live: Set<string>
        try {
          live = await liveRowIds()
        } catch (e) {
          // A failed liveness check must not degrade to "nothing is live": that would paint a
          // running row as STUCK, the mirror image of the bug this plugin exists to fix.
          res.statusCode = 500
          res.end(e instanceof Error ? e.message : String(e))
          return
        }
        const out: Record<string, LatestRun> = {}
        for (const [rowId, r] of await latestRuns()) {
          out[rowId] = {
            run_id: r.run_id,
            status: r.status,
            ended_by: r.ended_by || null,
            edits: numberOrNull(r.edits),
            turns: numberOrNull(r.turns),
            cost_usd: numberOrNull(r.cost_usd),
            end_time: r.end_time || null,
            live: live.has(rowId),
            bundle_path: r.bundle_path || null,
          }
        }
        sendJson(res, out)
      })

      // Connect strips the mount prefix, so req.url here is `/<row_id>/<run_id>` or
      // `/<row_id>/<run_id>/agent.log`.
      server.middlewares.use("/__dispatch/run", async (req, res) => {
        if (req.method !== "GET") {
          res.statusCode = 405
          res.end()
          return
        }
        const [rowId, runId, file, ...rest] = (req.url ?? "").split("?")[0].split("/").filter(Boolean)
        if (!rowId || !runId || !SAFE_ID.test(rowId) || !SAFE_ID.test(runId) || rest.length) {
          res.statusCode = 400
          res.end("expected /__dispatch/run/<row_id>/<run_id>[/agent.log]")
          return
        }
        const dir = path.join(RESULTS, rowId, runId)

        if (file === "agent.log") {
          const log = await readText(path.join(dir, "agent.log"))
          res.statusCode = log === null ? 404 : 200
          res.setHeader("Content-Type", "text/plain; charset=utf-8")
          res.end(log ?? "")
          return
        }
        if (file !== undefined) {
          res.statusCode = 404
          res.end()
          return
        }

        const [status, selfReportText, brief, agentLog] = await Promise.all([
          readJson(path.join(dir, "status.json")),
          readText(path.join(dir, "self-report.txt")),
          readText(path.join(dir, "brief.txt")),
          readText(path.join(dir, "agent.log")),
        ])
        sendJson(res, {
          ...(typeof status === "object" && status !== null ? status : {}),
          self_report_text: selfReportText,
          brief,
          agent_log_tail: tail(agentLog, LOG_TAIL_LINES),
        })
      })
    },
  }
}
