import { Marked, type Tokens } from "marked"
import { useEffect, useMemo, useState } from "react"

import { Code } from "@/components/Code"
import { MessageCard } from "@/components/MessageCard"
import { Button } from "@/components/ui/button"
import { Separator } from "@/components/ui/separator"
import { Textarea } from "@/components/ui/textarea"
import {
  fetchRound,
  fetchRoundTopics,
  isAnswered,
  submitAnswer,
  type Answer,
  type Round,
  type RoundOption,
  type RoundQuestion,
} from "@/lib/grill"
import { cn } from "@/lib/utils"

const md = new Marked()

/**
 * The wizard's dev-only entry points (kantord/toylang#34). Neither `App.tsx` nor `Markdown.tsx`
 * imports this module statically -- it only reaches the bundle through the `import.meta.env.DEV`
 * dynamic import in App.tsx's `GrillWizardRoute`, matching MailApp's tree-shaking pattern,
 * so `vite build` never ships it.
 */
export function GrillIndexPage() {
  const [topics, setTopics] = useState<string[] | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    fetchRoundTopics()
      .then(setTopics)
      .catch((e) => setError(e instanceof Error ? e.message : String(e)))
  }, [])

  return (
    <div className="max-w-lg space-y-3">
      <h1 className="text-lg font-semibold">Grilling rounds</h1>
      {error && <p className="text-sm text-destructive">{error}</p>}
      {!error && !topics && <p className="text-sm text-muted-foreground">Loading...</p>}
      {topics?.length === 0 && (
        <p className="text-sm text-muted-foreground">
          No rounds waiting in <code>docs/.grill/</code>.
        </p>
      )}
      {topics && topics.length > 0 && (
        <ul className="space-y-1">
          {topics.map((t) => (
            <li key={t}>
              <a href={`#/grill-wizard/${encodeURIComponent(t)}`} className="text-sm underline">
                {t}
              </a>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}

type Screen = { kind: "intro" } | { kind: "question"; index: number } | { kind: "summary" }

export function GrillWizardPage({ topic }: { topic: string }) {
  const [round, setRound] = useState<Round | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [answers, setAnswers] = useState<Record<string, Answer>>({})
  const [step, setStep] = useState(0)
  const [submitting, setSubmitting] = useState(false)
  const [submitError, setSubmitError] = useState<string | null>(null)
  const [submitted, setSubmitted] = useState(false)

  useEffect(() => {
    fetchRound(topic)
      .then(setRound)
      .catch((e) => setError(e instanceof Error ? e.message : String(e)))
  }, [topic])

  const screens = useMemo<Screen[]>(() => {
    if (!round) return []
    const qs: Screen[] = round.questions.map((_, index) => ({ kind: "question", index }))
    return [...(round.intro ? [{ kind: "intro" } as Screen] : []), ...qs, { kind: "summary" }]
  }, [round])

  if (error) {
    return (
      <div className="max-w-lg space-y-2">
        <p className="text-sm text-destructive">{error}</p>
        <a href="#/grill-wizard" className="text-sm underline">
          back to rounds
        </a>
      </div>
    )
  }
  if (!round) return <p className="text-sm text-muted-foreground">Loading...</p>

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
    } catch (e) {
      setSubmitError(e instanceof Error ? e.message : String(e))
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="max-w-2xl space-y-4">
      <header className="space-y-2">
        <h1 className="text-lg font-semibold">{topic}</h1>
        {screen.kind === "question" && (
          <>
            <p className="text-xs text-muted-foreground">
              Question {screen.index + 1} of {questionScreens}
            </p>
            <div className="h-1 w-full rounded-full bg-muted">
              <div
                className="h-1 rounded-full bg-primary transition-all"
                style={{ width: `${((screen.index + 1) / questionScreens) * 100}%` }}
              />
            </div>
          </>
        )}
      </header>

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
          <a href="#/grill-wizard" className="underline">
            back to rounds
          </a>
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

const SECTION: Record<"Background" | "Thesis" | "Question", { border: string; text: string }> = {
  Background: { border: "border-sky-500", text: "text-sky-700 dark:text-sky-300" },
  Thesis: { border: "border-violet-500", text: "text-violet-700 dark:text-violet-300" },
  // Reuses MessageCard's "question" flow color: the direct ask here is the same concept as an
  // inbox annotation's `question` flow, just rendered full-page instead of as a compact row.
  Question: { border: "border-fuchsia-500", text: "text-fuchsia-700 dark:text-fuchsia-300" },
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
  const ready = isAnswered(q, answer)

  return (
    <div className="space-y-4">
      <MessageCard flow={q.flow ?? "question"} note={q.title} />
      {q.background && <Section label="Background" markdown={q.background} />}
      {q.thesis && <Section label="Thesis" markdown={q.thesis} />}
      <Section label="Question" markdown={q.question} />

      {(q.options?.length || q.freeText) && (
        <div className="space-y-2">
          <div className="text-sm font-bold">Your answer</div>

          {q.options && q.options.length > 0 && (
            <div className="grid gap-3 sm:grid-cols-2">
              {q.options.map((o) => (
                <OptionCard
                  key={o.label}
                  option={o}
                  selected={selected === o.label}
                  onSelect={() => onAnswer({ optionLabel: o.label })}
                />
              ))}
            </div>
          )}

          {q.freeText && (
            <Textarea
              value={freeText}
              onChange={(e) => onAnswer({ freeText: e.target.value })}
              placeholder={typeof q.freeText === "string" ? q.freeText : "Type your answer..."}
              className={cn(!q.options?.length && "border-fuchsia-500/60")}
            />
          )}
        </div>
      )}

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
        "flex w-full min-w-0 flex-col space-y-2 rounded-lg border p-3 text-left transition-colors",
        selected ? "border-primary ring-2 ring-primary/30 bg-primary/5" : "border-border hover:bg-muted/50",
      )}
    >
      <div className={cn("text-sm", selected ? "font-bold" : "font-medium")}>{option.label}</div>
      <p className="text-xs text-muted-foreground">{option.description}</p>
      {option.preview && <Code code={option.preview} lang={option.previewLang ?? "toylang"} className="text-[11px]" />}
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
