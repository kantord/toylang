import { mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"

import type { Plugin } from "vite"

interface AnnotationRecord {
  page: string
  block: number
  original: string
  edited: string
}

interface Inbox {
  last_edit: string
  records: AnnotationRecord[]
}

/**
 * Dev-only autosave endpoint for annotations mode. `apply: "serve"` keeps it (and the
 * docs/.annotations directory it writes) out of `vite build` entirely, matching the issue's
 * "local dev server only" constraint.
 */
export function annotationsInbox(): Plugin {
  const dir = path.resolve(import.meta.dirname, "..", "..", "docs", ".annotations")
  const file = path.join(dir, "inbox.json")

  return {
    name: "annotations-inbox",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use("/__annotations/save", (req, res) => {
        if (req.method !== "POST") {
          res.statusCode = 405
          res.end()
          return
        }
        let body = ""
        req.on("data", (chunk) => (body += chunk))
        req.on("end", async () => {
          try {
            const { page, block, original, edited } = JSON.parse(body) as AnnotationRecord
            await mkdir(dir, { recursive: true })
            let inbox: Inbox = { last_edit: "", records: [] }
            try {
              inbox = JSON.parse(await readFile(file, "utf8"))
            } catch {
              // No inbox yet, or the coordinator just cleared it -- start fresh either way.
            }
            const records = inbox.records.filter((r) => !(r.page === page && r.block === block))
            records.push({ page, block, original, edited })
            const next: Inbox = { last_edit: new Date().toISOString(), records }
            await writeFile(file, JSON.stringify(next, null, 2))
            res.statusCode = 204
            res.end()
          } catch (e) {
            res.statusCode = 400
            res.end(e instanceof Error ? e.message : String(e))
          }
        })
      })
    },
  }
}
