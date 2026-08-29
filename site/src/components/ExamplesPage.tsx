import { CaseTree } from "@/components/CaseTree"
import { Code } from "@/components/Code"
import { RunPanel } from "@/components/RunPanel"
import { Badge } from "@/components/ui/badge"
import { Separator } from "@/components/ui/separator"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { BACKENDS, type Case, type CaseSummary } from "@/lib/corpus"
import { PAGES } from "@/lib/docs"
import { exampleHref } from "@/lib/pageData"
import { withBase } from "@/lib/route"

/**
 * The corpus browser: every program in the test corpus, and the code each backend compiles it
 * to. The whole site grew out of this page, which now lives under Examples. `current` and
 * `index` come from lib/pageData.ts -- the full case being shown, and the name/type summary of
 * every other case for the sidebar (kantord/toylang#50: not the whole corpus, which carries
 * seven backends' worth of emitted code no single page shows).
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
  const firstEuler = PAGES.find((p) => p.section === "examples")

  return (
    <div className="grid min-h-0 flex-1 gap-6 lg:grid-cols-[300px_minmax(0,1fr)]">
      <aside className="lg:sticky lg:top-6 lg:h-[calc(100vh-11rem)]">
        <CaseTree cases={index} selected={current.name} hrefFor={(name) => withBase(exampleHref(name))} />
      </aside>

      <main className="min-w-0 space-y-6">
        <p className="max-w-2xl text-sm text-muted-foreground">
          Every program in the test corpus, and the code each of the seven backends compiles it
          to. The same programs run on all seven and have to agree; what you see here is what
          that agreement is made of.
          {firstEuler && (
            <>
              {" "}
              Also here, off to the side:{" "}
              <a href={withBase("/examples/euler/")} className="underline">
                Project Euler solutions
              </a>{" "}
              (spoiler warning).
            </>
          )}
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
