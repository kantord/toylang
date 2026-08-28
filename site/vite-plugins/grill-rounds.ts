import { readdir, readFile } from "node:fs/promises"
import path from "node:path"

import type { Plugin } from "vite"
import { parse } from "yaml"

/**
 * Dev-only endpoints backing the grilling wizard (kantord/toylang#34): a round is a YAML file
 * the coordinator writes to `docs/.grill/<topic>.round.yaml` while the dev server is already
 * running, so it is read fresh on every request rather than through `import.meta.glob` (which
 * resolves file lists at transform time and would need a restart to notice a new topic).
 * `apply: "serve"` keeps this, like `annotations-inbox.ts`, out of `vite build` entirely.
 */
export function grillRounds(): Plugin {
  const dir = path.resolve(import.meta.dirname, "..", "..", "docs", ".grill")

  return {
    name: "grill-rounds",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use("/__grill/rounds", async (req, res) => {
        if (req.method !== "GET") {
          res.statusCode = 405
          res.end()
          return
        }
        let topics: string[] = []
        try {
          topics = (await readdir(dir))
            .filter((f) => f.endsWith(".round.yaml"))
            .map((f) => f.slice(0, -".round.yaml".length))
            .sort()
        } catch {
          // No docs/.grill directory yet -- no rounds, not an error.
        }
        res.statusCode = 200
        res.setHeader("Content-Type", "application/json")
        res.end(JSON.stringify({ topics }))
      })

      server.middlewares.use("/__grill/round", async (req, res) => {
        if (req.method !== "GET") {
          res.statusCode = 405
          res.end()
          return
        }
        const query = new URLSearchParams((req.url ?? "").split("?")[1] ?? "")
        const topic = query.get("topic") ?? ""
        // A round file is coordinator-controlled, but it still resolves through `path.join` and
        // gets checked against the directory it must stay under -- a topic is a URL query
        // parameter, and this endpoint has no other access control in front of it.
        const file = path.join(dir, `${topic}.round.yaml`)
        if (!topic || path.dirname(file) !== dir) {
          res.statusCode = 400
          res.end("invalid topic")
          return
        }
        let text: string
        try {
          text = await readFile(file, "utf8")
        } catch {
          res.statusCode = 404
          res.end(`no round for topic "${topic}"`)
          return
        }
        let round: unknown
        try {
          round = parse(text)
        } catch (e) {
          res.statusCode = 400
          res.end(`${file} is not valid YAML: ${e instanceof Error ? e.message : String(e)}`)
          return
        }
        if (typeof round !== "object" || round === null || !Array.isArray((round as { questions?: unknown }).questions)) {
          res.statusCode = 400
          res.end(`${file} needs a top-level "questions" list`)
          return
        }
        res.statusCode = 200
        res.setHeader("Content-Type", "application/json")
        res.end(JSON.stringify({ topic, ...(round as object) }))
      })
    },
  }
}
