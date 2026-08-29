import { CaseTree } from "@/components/CaseTree"
import { Code } from "@/components/Code"
import { RunPanel } from "@/components/RunPanel"
import { Badge } from "@/components/ui/badge"
import { Separator } from "@/components/ui/separator"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { BACKENDS, type Case, type CaseSummary } from "@/lib/corpus"
import { href, PAGES } from "@/lib/docs"
import { exampleHref } from "@/lib/pageData"
import { withBase } from "@/lib/route"

/**
 * The corpus browser: every program in the test corpus, and the code each backend compiles it
 * to. The whole site grew out of this page, which now lives under Examples. `current` and
 * `index` come from lib/pageData.ts -- the full case being shown, and the name/type summary of
 * every other case for the sidebar (kantord/toylang#50: not the whole corpus, which carries
 * seven backends' worth of emitted code no single page shows).
 *
 * The Examples sidebar has two folders (kantord/toylang#70): the corpus tree above, and the
 * Euler stream below it. They stay two folders rather than one merged tree because the corpus
 * is browsed by AST shape and the Euler stream is browsed in solving order -- forcing them into
 * one tree would lose whichever ordering isn't the tree's.
 */
export function ExamplesPage({
  current,
  index,
  backends,
}: {
  current: Case
  index: CaseSummary[]
  backends: string[]
}) {
  const eulerPages = PAGES.filter((p) => p.section === "examples")

  return (
    <div className="grid min-h-0 flex-1 gap-6 lg:grid-cols-[300px_minmax(0,1fr)]">
      <aside className="flex flex-col gap-4 lg:sticky lg:top-6 lg:h-[calc(100vh-11rem)]">
        <div className="flex min-h-0 flex-1 flex-col gap-2">
          <div className="shrink-0 text-xs font-medium uppercase tracking-wide text-muted-foreground">
            Test cases
          </div>
          <div className="min-h-0 flex-1">
            <CaseTree
              cases={index}
              selected={current.name}
              hrefFor={(name) => withBase(exampleHref(name))}
            />
          </div>
        </div>

        {eulerPages.length > 0 && (
          <div className="shrink-0 space-y-1 border-t pt-3">
            <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
              Coding puzzles
            </div>
            <nav className="max-h-40 space-y-0.5 overflow-y-auto">
              {eulerPages.map((p) => (
                <a
                  key={p.path}
                  href={withBase(href(p))}
                  className="block truncate rounded px-2 py-1 text-sm text-muted-foreground hover:bg-muted hover:text-foreground"
                >
                  {p.title}
                </a>
              ))}
            </nav>
          </div>
        )}
      </aside>

      <main className="min-w-0 space-y-6">
        <p className="max-w-2xl text-sm text-muted-foreground">
          Every program in the test corpus, and the code each of the seven backends compiles it
          to. The same programs run on all seven and have to agree; what you see here is what
          that agreement is made of.
        </p>

        <section className="space-y-3">
          <div className="flex flex-wrap items-center gap-2">
            <h2 className="font-mono text-lg font-medium">{current.name}</h2>
            <Badge variant="secondary" className="font-mono text-[11px]">
              {current.resultType}
            </Badge>
            {current.inputType && (
              <Badge variant="outline" className="font-mono text-[11px]">
                reads {current.inputType}
              </Badge>
            )}
            {current.expect.kind === "refusal" && (
              <Badge variant="destructive">every backend refuses</Badge>
            )}
          </div>
          <Code code={current.program} lang="toylang" />
        </section>

        <section className="grid gap-4 md:grid-cols-2">
          {current.input !== null && (
            <div className="space-y-2">
              <h3 className="text-xs font-medium text-muted-foreground">Input</h3>
              <Code code={current.input} lang="json" />
            </div>
          )}
          <div className="space-y-2">
            <h3 className="text-xs font-medium text-muted-foreground">
              {current.expect.kind === "output" ? "Expected output" : "Expected outcome"}
            </h3>
            <Code
              code={
                current.expect.kind === "output"
                  ? current.expect.value
                  : "Every backend refuses to run this."
              }
              lang="text"
            />
          </div>
        </section>

        <Separator />

        <section className="space-y-3">
          <h3 className="text-sm font-medium">Compiled to</h3>
          <Tabs defaultValue={backends[0]}>
            <TabsList>
              {backends.map((name) => (
                <TabsTrigger key={name} value={name}>
                  {BACKENDS[name]?.label ?? name}
                </TabsTrigger>
              ))}
            </TabsList>
            {backends.map((name) => (
              <TabsContent key={name} value={name} className="space-y-3">
                <p className="text-xs text-muted-foreground">{BACKENDS[name]?.note}</p>
                <Code code={current.emitted[name]} lang={BACKENDS[name]?.lang ?? "text"} />
              </TabsContent>
            ))}
          </Tabs>
        </section>

        <Separator />

        <section>
          <RunPanel current={current} />
        </section>
      </main>
    </div>
  )
}
