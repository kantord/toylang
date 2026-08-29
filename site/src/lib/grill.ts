/**
 * The grilling wizard's round data (kantord/toylang#34): a structured file the coordinator
 * writes to `docs/.grill/<topic>.round.yaml`, served by the dev-only `grill-rounds` vite plugin
 * (server-side YAML parsing keeps the `yaml` package's parsing off the critical path here, and
 * keeps this module free of anything that would need a build-time glob over a gitignored dir).
 * Split out of components so the fetch helpers stay testable without React.
 */

import type { FlowType } from "@/components/MessageCard"

export interface RoundOption {
  label: string
  description: string
  /** Full code as it would look under this option -- the wizard's reason to exist over a
   *  terminal dialog (kantord/toylang#34: "real code, real room"). */
  preview?: string
  previewLang?: string
}

export interface RoundQuestion {
  id: string
  title: string
  /** Which flow badge the question reads as (kantord/toylang#34 design-system comment: "flow
   *  types visually distinct"), reusing the same taxonomy MessageCard uses for annotations --
   *  `escalation` for something that needs the maintainer's ratification, `question` for a
   *  direct decision. Defaults to `question`. */
  flow?: FlowType
  /** Context sections, each markdown, each rendered as its own color-coded left-bordered block
   *  per the design-system comment. All optional except `question`, which is always present and
   *  always the bold line the maintainer answers. */
  background?: string
  thesis?: string
  question: string
  options?: RoundOption[]
  /** `true` for a free-text box with a generic placeholder, a string to use as the placeholder,
   *  or omitted for none. When a question has no `options`, free text is its only answer
   *  mechanism and is required. */
  freeText?: boolean | string
}

export interface Round {
  topic: string
  intro?: string
  questions: RoundQuestion[]
}

async function json<T>(res: Response): Promise<T> {
  if (!res.ok) throw new Error(await res.text())
  return res.json() as Promise<T>
}

export function fetchRoundTopics(): Promise<string[]> {
  return fetch("/__grill/rounds")
    .then((r) => json<{ topics: string[] }>(r))
    .then((r) => r.topics)
}

export function fetchRound(topic: string): Promise<Round> {
  return fetch(`/__grill/round?topic=${encodeURIComponent(topic)}`).then((r) => json<Round>(r))
}

/** One question's answer, as the wizard state holds it before submit. */
export interface Answer {
  optionLabel: string | null
  freeText: string
}

export function isAnswered(q: RoundQuestion, a: Answer | undefined): boolean {
  if (!a) return false
  if (q.options?.length) return a.optionLabel !== null
  return a.freeText.trim() !== ""
}

/** Delivers one question's answer through the #30 inbox machinery (kantord/toylang#34): the
 *  page identifies the round file itself, so the coordinator can open exactly that file to see
 *  which question a block index names. `edited` is JSON, not prose, because the issue asks for
 *  answers "shaped so the coordinator can map answers to questions mechanically." */
export function submitAnswer(topic: string, index: number, q: RoundQuestion, a: Answer): Promise<void> {
  return fetch("/__annotations/save", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      page: `docs/.grill/${topic}.round.yaml`,
      block: index,
      original: q.title,
      edited: JSON.stringify({ option: a.optionLabel, notes: a.freeText || null }),
    }),
  }).then((res) => {
    if (!res.ok) throw new Error(`submit failed for question ${index + 1}: ${res.statusText}`)
  })
}
