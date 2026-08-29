/**
 * The mail app's AUTHORING-side data (kantord/toylang#41): the maintainer's span notes and
 * free-form compose messages, both served by the `/__annotations/note` and
 * `/__annotations/compose` endpoints (vite-plugins/annotations-inbox.ts). Split out of the
 * component so the fetch helpers stay testable without React, matching lib/grill.ts.
 */

export interface NoteRecord {
  page: string
  block: number
  anchor: string
  note: string
}

/** A message composed with no page or span behind it -- the mail app's Compose button. */
export interface ComposeRecord {
  id: string
  subject: string
  note: string
  created: string
}

export function fetchNotesAndComposed(): Promise<{ notes: NoteRecord[]; composed: ComposeRecord[] }> {
  return fetch("/__annotations/notes-all")
    .then((r) => (r.ok ? (r.json() as Promise<{ records: NoteRecord[]; composed: ComposeRecord[] }>) : { records: [], composed: [] }))
    .then(({ records, composed }) => ({ notes: records, composed }))
    .catch(() => ({ notes: [], composed: [] }))
}

export function sendCompose(subject: string, note: string): Promise<void> {
  return fetch("/__annotations/compose", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ subject, note }),
  }).then((res) => {
    if (!res.ok) throw new Error(`compose failed: ${res.statusText}`)
  })
}
