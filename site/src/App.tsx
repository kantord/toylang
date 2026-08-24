import { useEffect, useMemo, useState } from "react"

import { CaseList } from "@/components/CaseList"
import { Code } from "@/components/Code"
import { RunPanel } from "@/components/RunPanel"
import { Badge } from "@/components/ui/badge"
import { Separator } from "@/components/ui/separator"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { BACKENDS, loadCorpus, type Corpus } from "@/lib/corpus"

export default function App() {
  const [corpus, setCorpus] = useState<Corpus | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [selected, setSelected] = useState("")
  const [backend, setBackend] = useState("js")

  useEffect(() => {
    loadCorpus()
      .then((c) => {
        setCorpus(c)
        // The hash makes a case linkable, so a finding can be pointed at rather than described.
        const wanted = decodeURIComponent(location.hash.slice(1))
        setSelected(c.cases.some((x) => x.name === wanted) ? wanted : c.cases[0].name)
      })
      .catch((e) => setError(e instanceof Error ? e.message : String(e)))
  }, [])

  useEffect(() => {
    if (selected) history.replaceState(null, "", `#${encodeURIComponent(selected)}`)
  }, [selected])

  const current = useMemo(
    () => corpus?.cases.find((c) => c.name === selected) ?? null,
    [corpus, selected],
  )

  if (error) {
    return (
      <main className="mx-auto max-w-2xl p-10">
        <h1 className="text-lg font-semibold">The corpus did not load</h1>
        <p className="mt-2 text-sm text-muted-foreground">{error}</p>
      </main>
    )
  }

  if (!corpus || !current) {
    return <main className="p-10 text-sm text-muted-foreground">Loading the corpus...</main>
  }

  return (
    <div className="mx-auto flex min-h-screen max-w-[1500px] flex-col gap-6 p-6">
      <header className="space-y-1">
        <h1 className="text-xl font-semibold tracking-tight">toylang corpus</h1>
        <p className="text-sm text-muted-foreground">
          Every program in the test corpus, and the code each of the six backends compiles it to.
          The same programs run on all six and have to agree; what you see here is what that
          agreement is made of.
        </p>
      </header>

      <div className="grid min-h-0 flex-1 gap-6 lg:grid-cols-[300px_minmax(0,1fr)]">
        <aside className="lg:h-[calc(100vh-11rem)] lg:sticky lg:top-6">
          <CaseList cases={corpus.cases} selected={selected} onSelect={setSelected} />
        </aside>

        <main className="min-w-0 space-y-6">
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
            <Tabs value={backend} onValueChange={setBackend}>
              <TabsList>
                {corpus.backends.map((name) => (
                  <TabsTrigger key={name} value={name}>
                    {BACKENDS[name]?.label ?? name}
                  </TabsTrigger>
                ))}
              </TabsList>
              {corpus.backends.map((name) => (
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
    </div>
  )
}
