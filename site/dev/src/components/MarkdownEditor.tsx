import type { Editor } from "@tiptap/core"
import { EditorContent, useEditor } from "@tiptap/react"
import StarterKit from "@tiptap/starter-kit"
import { Markdown, type MarkdownStorage } from "tiptap-markdown"

import { cn } from "@/lib/utils"

/** `Editor.storage` is a plain index type (`Record<string, any>`) that isn't narrowed by which
 *  extensions were actually passed to `useEditor` -- this is the documented shape the `Markdown`
 *  extension (tiptap-markdown) registers itself under, not a guess. */
function getMarkdown(editor: Editor): string {
  return (editor.storage as unknown as { markdown: MarkdownStorage }).markdown.getMarkdown()
}

/**
 * Thin Tiptap wrapper (kantord/toylang#grill-forest): markdown in via `content`, markdown out via
 * `editor.storage.markdown.getMarkdown()` (the tiptap-markdown extension). `content` only seeds
 * the editor on mount -- the empty `[]` deps array to `useEditor` is what makes that "once," so a
 * background data refresh feeding this component a fresh `content` prop can never stomp what the
 * human is mid-typing.
 */
export function MarkdownEditor({
  content,
  onChange,
  onFocus,
  className,
}: {
  content: string
  onChange: (markdown: string) => void
  onFocus?: () => void
  className?: string
}) {
  const editor = useEditor(
    {
      extensions: [StarterKit, Markdown],
      content,
      onUpdate: ({ editor }) => onChange(getMarkdown(editor)),
      onFocus: () => onFocus?.(),
      editorProps: {
        // `docs-prose` (src/index.css) is this repo's own hand-rolled heading/list/paragraph
        // styling -- there's no Tailwind typography plugin installed, so Tailwind's `prose`
        // utility classes don't exist here.
        attributes: {
          class: "docs-prose min-h-24 text-sm focus:outline-none",
        },
      },
    },
    [],
  )

  return (
    <div className={cn("rounded-md border p-2", className)}>
      <EditorContent editor={editor} />
    </div>
  )
}
