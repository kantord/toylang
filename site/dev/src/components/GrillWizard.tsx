import { Marked, type Tokens } from "marked"
import { useMemo, useState } from "react"

import { Code } from "@/components/Code"
import { FLOW, MessageCard } from "@dev/components/MessageCard"
import { Button } from "@/components/ui/button"
import { Separator } from "@/components/ui/separator"
import { Textarea } from "@/components/ui/textarea"
import { isAnswered, submitAnswer, type Answer, type Round, type RoundOption, type RoundQuestion } from "@dev/lib/grill"
import { cn } from "@/lib/utils"

const md = new Marked()

type Screen = { kind: "intro" } | { kind: "question"; index: number } | { kind: "summary" }

/**
 * A grilling round's own reading-pane body (kantord/toylang#52): rounds are a type of mail now,
 * not a route of their own -- the mail app fetches the round and opens this directly in place of
 * MessageCard when the item is a round, same as any other message body. One question per screen,
 * previews, a summary before an explicit submit, all unchanged from the original wizard
 * (kantord/toylang#34); only the page chrome (its own route, its own "back to rounds" link) is
 * gone, folded into the mail item it now is.
 */
export function GrillRoundReader({
  topic,
  round,
  onAllAnswered,
}: {
  topic: string
  round: Round
  onAllAnswered: () => void
}) {
  const [answers, setAnswers] = useState<Record<string, Answer>>({})
  const [step, setStep] = useState(0)
  const [submitting, setSubmitting] = useState(false)
  const [submitError, setSubmitError] = useState<string | null>(null)
  const [submitted, setSubmitted] = useState(false)

  const screens = useMemo<Screen[]>(() => {
    const qs: Screen[] = round.questions.map((_, index) => ({ kind: "question", index }))
    return [...(round.intro ? [{ kind: "intro" } as Screen] : []), ...qs, { kind: "summary" }]
  }, [round])

  const screen = screens[step]
  const questionScreens = screens.filter((s) => s.kind === "question").length

  const setAnswer = (id: string, next: Partial<Answer>) =>
    setAnswers((prev) => ({
      ...prev,
      [id]: { optionLabel: prev[id]?.optionLabel ?? null, freeText: prev[id]?.freeText ?? "", ...next },
    }))

  const submit = async () => {
    setSubmitting(true)
    setSubmitError(null)
    try {
      for (let i = 0; i < round.questions.length; i++) {
        const q = round.questions[i]
        await submitAnswer(topic, i, q, answers[q.id] ?? { optionLabel: null, freeText: "" })
      }
      setSubmitted(true)
      onAllAnswered()
    } catch (e) {
      setSubmitError(e instanceof Error ? e.message : String(e))
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="max-w-2xl space-y-4">
      {screen.kind === "question" && (
        <header className="space-y-2">
          <p className="text-xs text-muted-foreground">
            Question {screen.index + 1} of {questionScreens}
          </p>
          <div className="h-1 w-full rounded-full bg-muted">
            <div
              className="h-1 rounded-full bg-primary transition-all"
              style={{ width: `${((screen.index + 1) / questionScreens) * 100}%` }}
            />
          </div>
        </header>
      )}

      {screen.kind === "intro" && (
        <IntroScreen round={round} onBegin={() => setStep(step + 1)} />
      )}

      {screen.kind === "question" && (
        <QuestionScreen
          q={round.questions[screen.index]}
          answer={answers[round.questions[screen.index].id]}
          onAnswer={(next) => setAnswer(round.questions[screen.index].id, next)}
          onBack={step > 0 ? () => setStep(step - 1) : undefined}
          onNext={() => setStep(step + 1)}
        />
      )}

      {screen.kind === "summary" && !submitted && (
        <SummaryScreen
          round={round}
          answers={answers}
          onBack={() => setStep(step - 1)}
          onSubmit={submit}
          submitting={submitting}
          submitError={submitError}
        />
      )}

      {submitted && (
        <div className="space-y-2 rounded-md border border-emerald-500/40 bg-emerald-500/5 p-4 text-sm">
          <p className="font-semibold">Delivered.</p>
          <p className="text-muted-foreground">
            Every answer is in the coordinator's inbox, keyed to this round -- no quiet-period
            wait, this was an explicit submit.
          </p>
        </div>
      )}
    </div>
  )
}

function IntroScreen({ round, onBegin }: { round: Round; onBegin: () => void }) {
  return (
    <div className="space-y-4">
      {round.intro && <GrillMarkdown text={round.intro} />}
      <Button onClick={onBegin}>Begin ({round.questions.length} question{round.questions.length === 1 ? "" : "s"})</Button>
    </div>
  )
}

// Borders come from MessageCard's FLOW palette (kantord/toylang#43) so the two color-block
// renderers can't drift apart: Background borrows "status" (sky), Thesis borrows "round"
// (violet -- a wizard question is grill mail, same as a round), Question borrows "question"
// itself. The block *shape* still diverges on purpose: MessageCard packs several labeled
// sentences under one badge for a dense inbox row, while a wizard section is one full-page
// prose block per label, so it keeps its own uppercase-label-then-markdown layout instead of
// MessageCard's regex-driven sentence splitting.
const SECTION: Record<"Background" | "Thesis" | "Question", { border: string; text: string }> = {
  Background: { border: FLOW.status.border, text: "text-sky-700 dark:text-sky-300" },
  Thesis: { border: FLOW.round.border, text: "text-violet-700 dark:text-violet-300" },
  Question: { border: FLOW.question.border, text: "text-fuchsia-700 dark:text-fuchsia-300" },
}

function Section({ label, markdown }: { label: keyof typeof SECTION; markdown: string }) {
  const s = SECTION[label]
  return (
    <div className={cn("space-y-2 rounded-sm border-l-4 py-1 pl-3", s.border)}>
      <div className={cn("text-xs font-semibold uppercase tracking-wide", s.text)}>{label}</div>
      <GrillMarkdown text={markdown} className={label === "Question" ? "font-semibold" : undefined} />
    </div>
  )
}

function QuestionScreen({
  q,
  answer,
  onAnswer,
  onBack,
  onNext,
}: {
  q: RoundQuestion
  answer: Answer | undefined
  onAnswer: (next: Partial<Answer>) => void
  onBack: (() => void) | undefined
  onNext: () => void
}) {
  const selected = answer?.optionLabel ?? null
  const freeText = answer?.freeText ?? ""
  const ready = isAnswered(answer)
  const hasOptions = !!q.options?.length

  return (
    <div className="space-y-4">
      <MessageCard flow={q.flow ?? "question"} note={q.title} />
      {q.background && <Section label="Background" markdown={q.background} />}
      {q.thesis && <Section label="Thesis" markdown={q.thesis} />}
      <Section label="Question" markdown={q.question} />

      <div className="space-y-2">
        <div className="text-sm font-bold">Your answer</div>

        {hasOptions && (
          <div className="grid gap-3 sm:grid-cols-2">
            {q.options?.map((o) => (
              <OptionCard
                key={o.label}
                option={o}
                selected={selected === o.label}
                onSelect={() => onAnswer({ optionLabel: o.label })}
              />
            ))}
          </div>
        )}

        {/* Writing your own option is always available, even when the round author didn't add
            a free-text box (kantord/toylang#52) -- an option list is never the only door. */}
        <Textarea
          value={freeText}
          onChange={(e) => onAnswer({ freeText: e.target.value })}
          placeholder={typeof q.freeText === "string" ? q.freeText : hasOptions ? "Write your own option" : "Type your answer..."}
          className={cn(!hasOptions && "border-fuchsia-500/60")}
        />
      </div>

      <div className="flex justify-between pt-2">
        <Button variant="outline" onClick={onBack} disabled={!onBack}>
          Back
        </Button>
        <Button onClick={onNext} disabled={!ready}>
          Next
        </Button>
      </div>
    </div>
  )
}

function OptionCard({
  option,
  selected,
  onSelect,
}: {
  option: RoundOption
  selected: boolean
  onSelect: () => void
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      className={cn(
        "group flex w-full min-w-0 flex-col space-y-2 rounded-lg border p-3 text-left transition-colors",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40 sm:focus-visible:col-span-2",
        selected ? "border-primary ring-2 ring-primary/30 bg-primary/5 sm:col-span-2" : "border-border hover:bg-muted/50",
      )}
    >
      <div className={cn("text-sm", selected ? "font-bold" : "font-medium")}>{option.label}</div>
      <p className="text-xs text-muted-foreground">{option.description}</p>
      {option.preview && (
        <Code
          code={option.preview}
          lang={option.previewLang ?? "toylang"}
          className={selected ? "text-sm" : "text-[11px] group-focus-visible:text-sm"}
        />
      )}
    </button>
  )
}

function SummaryScreen({
  round,
  answers,
  onBack,
  onSubmit,
  submitting,
  submitError,
}: {
  round: Round
  answers: Record<string, Answer>
  onBack: () => void
  onSubmit: () => void
  submitting: boolean
  submitError: string | null
}) {
  return (
    <div className="space-y-4">
      <div className="text-sm font-bold">Review before you submit</div>
      <div className="space-y-3">
        {round.questions.map((q, i) => {
          const a = answers[q.id]
          return (
            <div key={q.id}>
              {i > 0 && <Separator className="mb-3" />}
              <div className="text-sm font-medium">{q.title}</div>
              <p className="text-sm text-muted-foreground">
                {a?.optionLabel ?? (a?.freeText ? null : "(unanswered)")}
                {a?.optionLabel && a?.freeText ? " -- " : ""}
                {a?.freeText}
              </p>
            </div>
          )
        })}
      </div>
      {submitError && <p className="text-sm text-destructive">{submitError}</p>}
      <div className="flex justify-between pt-2">
        <Button variant="outline" onClick={onBack} disabled={submitting}>
          Back
        </Button>
        <Button onClick={onSubmit} disabled={submitting}>
          {submitting ? "Submitting..." : "Submit"}
        </Button>
      </div>
    </div>
  )
}

/** Markdown with real code fences (kantord/toylang#34: "full code blocks"), split the same way
 *  `lib/blocks.ts` splits a docs page -- a fence gets `Code`'s shiki highlighting, everything
 *  else goes through marked's own HTML. No fragment protocol here: round files are ephemeral and
 *  the fence harness does not check them (grill-via-annotations skill), so any language tag is
 *  just illustration. */
function GrillMarkdown({ text, className }: { text: string; className?: string }) {
  const tokens = useMemo(() => md.lexer(text), [text])
  return (
    <div className={cn("docs-prose", className)}>
      {tokens.map((t, i) =>
        t.type === "code" ? (
          <Code key={i} code={(t as Tokens.Code).text} lang={(t as Tokens.Code).lang || "text"} />
        ) : (
          <div key={i} dangerouslySetInnerHTML={{ __html: md.parser([t]) }} />
        ),
      )}
    </div>
  )
}
