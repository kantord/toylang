import type { ReactNode } from "react"

import { agentLogHref, useDispatchRun, type RunBundle } from "@dev/lib/dispatch"

/**
 * One dispatch run's bundle (`~/.cache/toylang-simple-dispatch/results/<row>/<run>/`), reached
 * from a delegated card's run link: the status.json fields, the per-attempt table, the worker's
 * self-report, the brief it was given, and the tail of its log with the full log a click away.
 * This is the page a coordinator opens to answer "why did this row stop" without a shell.
 */
export function RunPage({ rowId, runId }: { rowId: string; runId: string }) {
  const { data, error, isPending } = useDispatchRun(rowId, runId)

  if (isPending) return <p className="text-sm text-muted-foreground">Loading...</p>
  if (error) return <p className="text-sm text-destructive">{error.message}</p>

  const facts: [string, unknown][] = [
    ["status", data.status],
    ["ended_by", data.ended_by],
    ["model", data.model],
    ["start_time", data.start_time],
    ["end_time", data.end_time],
    ["duration_s", data.duration_s],
    ["cost_usd", data.cost_usd],
    ["edits", data.edits],
    ["turns", data.turns],
    ["patch_path", data.patch_path],
    ["bundle_path", data.bundle_path],
  ]

  return (
    <div className="space-y-6 text-sm">
      <div>
        <a href="#/mail" className="text-xs text-muted-foreground hover:text-foreground">
          &larr; back to mail
        </a>
        <h2 className="mt-1 font-mono text-base font-semibold">
          {rowId} / {runId}
        </h2>
        {data.status === undefined && (
          <p className="text-xs text-muted-foreground">
            No status.json in this run's bundle -- runs before the bundle layout landed have none.
          </p>
        )}
      </div>

      <dl className="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1 font-mono text-xs">
        {facts.map(([k, v]) => (
          <Fact key={k} name={k} value={v} />
        ))}
      </dl>

      <Section title="Attempts">
        <AttemptsTable attempts={data.attempts} />
      </Section>

      <Section title="Self-report">
        <SelfReportView data={data} />
      </Section>

      <Section title="Brief">
        <Pre text={data.brief} />
      </Section>

      <Section
        title="Agent log (last 200 lines)"
        aside={
          <a href={agentLogHref(rowId, runId)} target="_blank" rel="noreferrer" className="text-xs text-primary hover:underline">
            full log
          </a>
        }
      >
        <Pre text={data.agent_log_tail} />
      </Section>
    </div>
  )
}

function Fact({ name, value }: { name: string; value: unknown }) {
  return (
    <>
      <dt className="text-muted-foreground">{name}</dt>
      <dd className="break-all">{value === undefined || value === null || value === "" ? "-" : String(value)}</dd>
    </>
  )
}

function Section({ title, aside, children }: { title: string; aside?: ReactNode; children: ReactNode }) {
  return (
    <section className="space-y-2">
      <div className="flex items-baseline justify-between">
        <h3 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">{title}</h3>
        {aside}
      </div>
      {children}
    </section>
  )
}

function Pre({ text }: { text: string | null }) {
  if (text === null) return <p className="text-xs text-muted-foreground">not in bundle</p>
  return (
    <pre className="max-h-[60vh] overflow-auto rounded-md border border-border bg-muted/30 p-3 font-mono text-xs leading-snug whitespace-pre-wrap">
      {text}
    </pre>
  )
}

function AttemptsTable({ attempts }: { attempts: RunBundle["attempts"] }) {
  if (!attempts?.length) return <p className="text-xs text-muted-foreground">no attempts recorded</p>
  return (
    <div className="overflow-x-auto">
      <table className="w-full border-collapse text-xs">
        <thead>
          <tr className="border-b border-border text-left text-muted-foreground">
            <th className="py-1 pr-3 font-medium">n</th>
            <th className="py-1 pr-3 font-medium">ending</th>
            <th className="py-1 pr-3 font-medium">moved</th>
            <th className="py-1 pr-3 font-medium">turns</th>
            <th className="py-1 font-medium">verify tail</th>
          </tr>
        </thead>
        <tbody>
          {attempts.map((a) => (
            <tr key={a.n} className="border-b border-border/50 align-top">
              <td className="py-1 pr-3 font-mono">{a.n}</td>
              <td className="py-1 pr-3 font-mono">{a.ending}</td>
              <td className="py-1 pr-3">{a.moved ? "yes" : "no"}</td>
              <td className="py-1 pr-3 font-mono">{a.turns}</td>
              <td className="py-1 font-mono whitespace-pre-wrap">{a.verify_tail}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

function SelfReportView({ data }: { data: RunBundle }) {
  const r = data.self_report
  return (
    <div className="space-y-2">
      {r ? (
        <dl className="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1 text-xs">
          <Fact name="blocker_kind" value={r.blocker_kind} />
          <Fact name="narrower_would_succeed" value={r.narrower_would_succeed ? "yes" : "no"} />
          <Fact name="explanation" value={r.explanation} />
        </dl>
      ) : (
        <p className="text-xs text-muted-foreground">no structured self-report</p>
      )}
      <Pre text={data.self_report_text} />
    </div>
  )
}
