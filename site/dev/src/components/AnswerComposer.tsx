import { useEffect, useRef, useState } from "react"

import { MarkdownEditor } from "@dev/components/MarkdownEditor"
import { Button } from "@/components/ui/button"
import { clearDraft, loadDraft, saveDraft } from "@dev/lib/draft"
import { submitForestAnswer, type ForestNode } from "@dev/lib/grillForest"
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
}

export function AnswerComposer({ topic, node, onSubmitted }: { topic: string; node: ForestNode; onSubmitted: () => void }) {
  const key = draftKey(topic, node.id)
  const initialDraft = useRef(loadDraft<SavedDraft | null>(key, null)).current
  const [boxes, setBoxes] = useState<BoxState[]>(() => initialDraft?.boxes ?? initialBoxes(node))
  const [selected, setSelected] = useState<number | null>(() => initialDraft?.selected ?? null)
  const [submitting, setSubmitting] = useState(false)
  const [submitError, setSubmitError] = useState<string | null>(null)

  useEffect(() => {
    saveDraft(key, { selected, boxes } satisfies SavedDraft)
  }, [key, selected, boxes])

  const setBoxContent = (index: number, content: string) =>
    setBoxes((prev) => prev.map((b, i) => (i === index ? { ...b, content, dirty: true } : b)))

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
            <MarkdownEditor content={box.content} onChange={(md) => setBoxContent(i, md)} onFocus={() => setSelected(i)} />
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
