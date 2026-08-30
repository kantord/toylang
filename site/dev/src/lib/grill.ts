/**
 * The grilling wizard's round data (kantord/toylang#34): a structured file the coordinator
 * writes to `docs/.grill/<topic>.round.yaml`, served by the dev-only `grill-rounds` vite plugin
 * (server-side YAML parsing keeps the `yaml` package's parsing off the critical path here, and
 * keeps this module free of anything that would need a build-time glob over a gitignored dir).
 * Split out of components so the fetch helpers stay testable without React.
 */

import type { FlowType } from "@dev/components/MessageCard"
import { saveToInbox } from "@dev/lib/annotations"

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
  /** A string to use as the free-text box's placeholder, or omitted for a generic one. The box
   *  itself is always shown regardless of this field (kantord/toylang#52: "writing my own
   *  option must always be available, not only where the round author added a free-text box") --
   *  this only customizes it. */
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

/** The inbox page identity a round's answers are saved under -- shared by `submitAnswer` and the
 *  mail app's read-state check (kantord/toylang#52), which needs to ask the same `page:block`
 *  keys `/__annotations/inbox-all` already answers for ordinary annotations. */
export function roundPagePath(topic: string): string {
  return `docs/.grill/${topic}.round.yaml`
}

/** One question's answer, as the wizard state holds it before submit. */
export interface Answer {
  optionLabel: string | null
  freeText: string
}

/** A question is answered by picking an option, or by writing a free-text answer -- the two
 *  are always both available (kantord/toylang#52), so either satisfies it. */
export function isAnswered(a: Answer | undefined): boolean {
  if (!a) return false
  return a.optionLabel !== null || a.freeText.trim() !== ""
}

/** Delivers one question's answer through the #30 inbox machinery (kantord/toylang#34): the
 *  page identifies the round file itself, so the coordinator can open exactly that file to see
 *  which question a block index names. `edited` is JSON, not prose, because the issue asks for
 *  answers "shaped so the coordinator can map answers to questions mechanically." */
export function submitAnswer(topic: string, index: number, q: RoundQuestion, a: Answer): Promise<void> {
  return saveToInbox(
    {
      page: roundPagePath(topic),
      block: index,
      original: q.title,
      edited: JSON.stringify({ option: a.optionLabel, notes: a.freeText || null }),
    },
    `submit failed for question ${index + 1}`,
  )
}
