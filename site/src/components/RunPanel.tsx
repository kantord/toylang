import { useEffect, useMemo, useState } from "react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"
import type { Case } from "@/lib/corpus"
import { runJs, type RunResult } from "@/lib/run-js"

/** Whether the JSON parses, so the Run button can say why it is disabled. */
function parse(text: string): { ok: true; value: unknown } | { ok: false; error: string } {
  try {
    return { ok: true, value: JSON.parse(text) }
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) }
  }
}

export function RunPanel({ current }: { current: Case }) {
  const [text, setText] = useState(current.input ?? "")
  const [result, setResult] = useState<RunResult | null>(null)

  // Editing one case and switching to another should not carry the edit across.
  useEffect(() => {
    setText(current.input ?? "")
    setResult(null)
  }, [current.name, current.input])

  const parsed = useMemo(() => parse(text), [text])
  const needsInput = current.input !== null
  const edited = needsInput && text !== current.input
  const canRun = !needsInput || parsed.ok

  const run = () => {
    setResult(runJs(current.emitted.js, needsInput && parsed.ok ? parsed.value : null))
  }

  const expected = current.expect.kind === "output" ? current.expect.value : null
  // Only meaningful against the input the corpus recorded. Once that is edited there is nothing
  // to compare to, and saying "differs" would be reporting the reader's own change back at them.
  const verdict =
    result?.ok && expected !== null && !edited
      ? result.output === expected
        ? "matches"
        : "differs"
      : null

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <div className="flex flex-wrap items-baseline justify-between gap-2">
          <h3 className="text-sm font-medium">Run the JavaScript here</h3>
          {current.expect.kind === "refusal" && (
            <span className="text-xs text-muted-foreground">
              This program is meant to be refused. Running it should throw.
            </span>
          )}
        </div>
        {needsInput ? (
          <>
            <label
              htmlFor="stdin"
              className="block font-mono text-xs text-muted-foreground"
            >
              stdin, which must be {current.inputType}
            </label>
            <Textarea
              id="stdin"
              value={text}
              onChange={(e) => setText(e.target.value)}
              spellCheck={false}
              rows={4}
              className="font-mono text-[13px]"
            />
            {!parsed.ok && (
              <p className="text-xs text-destructive">Not valid JSON: {parsed.error}</p>
            )}
          </>
        ) : (
          <p className="text-xs text-muted-foreground">
            This program reads no input, so there is nothing to edit.
          </p>
        )}
      </div>

      <div className="flex items-center gap-3">
        <Button onClick={run} disabled={!canRun} size="sm">
          Run
        </Button>
        {edited && (
          <span className="text-xs text-muted-foreground">
            Input edited, so there is nothing to compare against.
          </span>
        )}
        {verdict === "matches" && <Badge>Matches the recorded output</Badge>}
        {verdict === "differs" && <Badge variant="destructive">Differs from the corpus</Badge>}
      </div>

      {result && (
        <div className="space-y-2">
          <h4 className="text-xs font-medium text-muted-foreground">
            {result.ok ? "Output" : "Refused"}
          </h4>
          <pre
            className={`overflow-x-auto rounded-md border p-4 text-[13px] leading-relaxed ${
              result.ok ? "bg-muted/40" : "border-destructive/40 bg-destructive/5 text-destructive"
            }`}
          >
            <code>{result.ok ? result.output || "(no output)" : result.error}</code>
          </pre>
        </div>
      )}
    </div>
  )
}
