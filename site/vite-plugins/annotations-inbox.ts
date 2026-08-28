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

/** One maintainer-authored note (kantord/toylang#30): the AUTHORING side, wholly separate from
 *  the coordinator's INBOX above. `anchor` is the exact text the maintainer selected, matching
 *  the quoted-span format the coordinator's own annotations use. */
interface NoteRecord {
  page: string
  block: number
  anchor: string
  note: string
}

interface Notes {
  last_note: string
  records: NoteRecord[]
}

/**
 * Dev-only autosave endpoint for annotations mode. `apply: "serve"` keeps it (and the
 * docs/.annotations directory it writes) out of `vite build` entirely, matching the issue's
 * "local dev server only" constraint.
 */
async function readInbox(file: string): Promise<Inbox> {
  try {
    return JSON.parse(await readFile(file, "utf8"))
  } catch {
    // No inbox yet, or the coordinator just cleared it -- start fresh either way.
    return { last_edit: "", records: [] }
  }
}

async function readNotes(file: string): Promise<Notes> {
  try {
    return JSON.parse(await readFile(file, "utf8"))
  } catch {
    // No notes yet, or the coordinator just archived them -- start fresh either way.
    return { last_note: "", records: [] }
  }
}

function readBody(req: import("node:http").IncomingMessage): Promise<string> {
  return new Promise((resolve) => {
    let body = ""
    req.on("data", (chunk) => (body += chunk))
    req.on("end", () => resolve(body))
  })
}

export function annotationsInbox(): Plugin {
  const dir = path.resolve(import.meta.dirname, "..", "..", "docs", ".annotations")
  const file = path.join(dir, "inbox.json")
  const notesFile = path.join(dir, "notes.json")

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
            const inbox = await readInbox(file)
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

      // Rehydration for a reloaded page (kantord/toylang#28): the record for one block, if the
      // coordinator hasn't consumed it yet. Read-only, so a stale cache here never loses data --
      // the browser's localStorage draft, written on every keystroke, is the source of truth for
      // this same session, and this endpoint only covers what a fresh browser wouldn't have.
      server.middlewares.use("/__annotations/inbox", async (req, res) => {
        if (req.method !== "GET") {
          res.statusCode = 405
          res.end()
          return
        }
        const query = new URLSearchParams((req.url ?? "").split("?")[1] ?? "")
        const page = query.get("page")
        const block = Number(query.get("block"))
        const inbox = await readInbox(file)
        const record = inbox.records.find((r) => r.page === page && r.block === block) ?? null
        res.statusCode = 200
        res.setHeader("Content-Type", "application/json")
        res.end(JSON.stringify({ record }))
      })

      // Every reply record, for the inbox sidebar's read/unread sort (kantord/toylang#30): a
      // block with a record here is one the maintainer has already answered.
      server.middlewares.use("/__annotations/inbox-all", async (req, res) => {
        if (req.method !== "GET") {
          res.statusCode = 405
          res.end()
          return
        }
        const inbox = await readInbox(file)
        res.statusCode = 200
        res.setHeader("Content-Type", "application/json")
        res.end(JSON.stringify({ records: inbox.records }))
      })

      // The AUTHORING side (kantord/toylang#30): the maintainer's own notes back to the
      // coordinator, wholly separate from the INBOX reply above. One anchor per note; posting
      // again with the same page/block/anchor replaces it, same dedup rule as a reply.
      server.middlewares.use("/__annotations/note", async (req, res) => {
        const query = new URLSearchParams((req.url ?? "").split("?")[1] ?? "")
        if (req.method === "GET") {
          const page = query.get("page")
          const block = Number(query.get("block"))
          const notes = await readNotes(notesFile)
          const records = notes.records.filter((r) => r.page === page && r.block === block)
          res.statusCode = 200
          res.setHeader("Content-Type", "application/json")
          res.end(JSON.stringify({ records }))
          return
        }
        if (req.method !== "POST") {
          res.statusCode = 405
          res.end()
          return
        }
        try {
          const { page, block, anchor, note } = JSON.parse(await readBody(req)) as NoteRecord
          await mkdir(dir, { recursive: true })
          const notes = await readNotes(notesFile)
          const records = notes.records.filter((r) => !(r.page === page && r.block === block && r.anchor === anchor))
          records.push({ page, block, anchor, note })
          const next: Notes = { last_note: new Date().toISOString(), records }
          await writeFile(notesFile, JSON.stringify(next, null, 2))
          res.statusCode = 204
          res.end()
        } catch (e) {
          res.statusCode = 400
          res.end(e instanceof Error ? e.message : String(e))
        }
      })

      // Every note, for the sidebar's "your notes" section.
      server.middlewares.use("/__annotations/notes-all", async (req, res) => {
        if (req.method !== "GET") {
          res.statusCode = 405
          res.end()
          return
        }
        const notes = await readNotes(notesFile)
        res.statusCode = 200
        res.setHeader("Content-Type", "application/json")
        res.end(JSON.stringify({ records: notes.records }))
      })
    },
  }
}
