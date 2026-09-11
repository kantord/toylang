import { useEffect, useRef, useState } from "react"

import { MarkdownEditor } from "@dev/components/MarkdownEditor"
import { Button } from "@/components/ui/button"
import { clearDraft, loadDraft, saveDraft } from "@dev/lib/draft"
import { clearPendingAnswer, submitForestAnswer, type ForestNode } from "@dev/lib/grillForest"
import { cn } from "@/lib/utils"

/** Up to 5 editors: one per agent-proposed option (kantord/toylang#grill-forest, capped at 4) plus
 *  always one empty box to write a new option from scratch -- never fewer than the blank box, even
 *  when a node has no options at all. `dirty` drives `wasEdited`: it flips true the first time the
 *  box's content changes at all, rather than diffing final content against the pre-filled value --
 *  Tiptap's markdown round-trip could cosmetically reformat an untouched value and wrongly read as
 *  an edit. */
interface BoxState {
  label: string | null
  content: string
  dirty: boolean
}

function draftKey(topic: string, nodeId: string): string {
  return `toylang-grill-forest-draft:${topic}:${nodeId}`
}

function initialBoxes(node: ForestNode): BoxState[] {
  const fromOptions = (node.options ?? []).slice(0, 4).map((o) => ({ label: o.label, content: o.content, dirty: false }))
  return [...fromOptions, { label: null, content: "", dirty: false }]
}

interface SavedDraft {
  selected: number | null
  boxes: BoxState[]
  // Survives the remount a failed submit causes (see `submit`'s catch below): once
  // `submitForestAnswer` writes the local pending record, any poll-triggered re-render of
  // `GrillChain` swaps this component out for a "Sending..." bubble, even though nothing has
  // actually been confirmed yet -- on failure the pending record is rolled back so the composer
  // comes back, but as a fresh mount with no in-memory state, so the error itself has to live
  // here to survive that round-trip instead of vanishing with the old instance.
  lastError: string | null
}

export function AnswerComposer({ topic, node, onSubmitted }: { topic: string; node: ForestNode; onSubmitted: () => void }) {
  const key = draftKey(topic, node.id)
  const initialDraft = useRef(loadDraft<SavedDraft | null>(key, null)).current
  const [boxes, setBoxes] = useState<BoxState[]>(() => initialDraft?.boxes ?? initialBoxes(node))
  const [selected, setSelected] = useState<number | null>(() => initialDraft?.selected ?? null)
  const [submitting, setSubmitting] = useState(false)
  const [submitError, setSubmitError] = useState<string | null>(() => initialDraft?.lastError ?? null)

  useEffect(() => {
    saveDraft(key, { selected, boxes, lastError: submitError } satisfies SavedDraft)
  }, [key, selected, boxes, submitError])

  // Typing into a box selects it -- editing is a deliberate act. Focusing one (e.g. tabbing or
  // clicking in just to read it before deciding) deliberately does NOT select it on its own
  // anymore: it used to, which meant glancing at a box could silently swap which one Cmd/Ctrl+Enter
  // would submit. Clicking the box's own card (not its editor) still selects it explicitly.
  const setBoxContent = (index: number, content: string) => {
    setBoxes((prev) => prev.map((b, i) => (i === index ? { ...b, content, dirty: true } : b)))
    setSelected(index)
  }

  const selectedBox = selected !== null ? boxes[selected] : null
  const ready = !!selectedBox && selectedBox.content.trim() !== ""

  const submit = async () => {
    if (!selectedBox || !ready || submitting) return
    setSubmitting(true)
    setSubmitError(null)
    try {
      await submitForestAnswer(topic, node, {
        sourceOption: selectedBox.label,
        content: selectedBox.content,
        wasEdited: selectedBox.dirty,
      })
      clearDraft(key)
      onSubmitted()
    } catch (e) {
      // Roll the optimistic local record back: `submitForestAnswer` already wrote it before this
      // POST failed, and leaving it in place would make `GrillChain` swap to a "Sending..." bubble
      // with no way back to this composer or its error the moment any poll tick re-renders it --
      // the pending record existing is what drives that swap, not whether the send succeeded.
      clearPendingAnswer(topic, node.id)
      setSubmitError(e instanceof Error ? e.message : String(e))
    } finally {
      setSubmitting(false)
    }
  }

  // A bare digit shortcut per box was considered and dropped (two reviewers of the design flagged
  // it would hijack ordinary digit-typing inside these very editors); Cmd/Ctrl+Enter needs a
  // modifier, so it can't collide. The listener is attached once and reads the latest `submit`
  // through a ref so it doesn't get torn down and reattached on every keystroke.
  const submitRef = useRef(submit)
  submitRef.current = submit
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
        e.preventDefault()
        submitRef.current()
      }
    }
    document.addEventListener("keydown", onKey)
    return () => document.removeEventListener("keydown", onKey)
  }, [])

  return (
    <div className="space-y-3">
      <div className="grid gap-3 sm:grid-cols-2">
        {boxes.map((box, i) => (
          <div
            key={box.label ?? "__blank__"}
            onClick={() => setSelected(i)}
            className={cn(
              "space-y-2 rounded-lg border p-2 transition-colors",
              selected === i ? "border-primary ring-2 ring-primary/30 bg-primary/5" : "border-border hover:bg-muted/40",
            )}
          >
            <div className="text-xs font-medium text-muted-foreground">{box.label ?? "Write your own"}</div>
            <MarkdownEditor content={box.content} onChange={(md) => setBoxContent(i, md)} />
          </div>
        ))}
      </div>
      {submitError && <p className="text-sm text-destructive">Delivery failed: {submitError}. Your answer is kept -- retry.</p>}
      <div className="flex justify-end">
        <Button onClick={submit} disabled={!ready || submitting}>
          {submitting ? "Submitting..." : "Submit"}
        </Button>
      </div>
    </div>
  )
}
