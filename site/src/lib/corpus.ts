export type Expect =
  | { kind: "output"; value: string }
  | { kind: "refusal" }

export interface Case {
  name: string
  program: string
  /** The JSON fed to stdin, verbatim. Absent when the program reads no input. */
  input: string | null
  /** The type the program declares its input has, for a reader editing that input. */
  inputType: string | null
  /** Whether `input` here is a `lines` program's fixture text rather than a JSON document. */
  usesLines: boolean
  resultType: string
  /** Which AST shapes this program exercises, in dotted-path form (`arith.add`, `projection`). */
  nodeTypes: string[]
  expect: Expect
  emitted: Record<string, string>
}

export interface Corpus {
  backends: string[]
  cases: Case[]
}

/** How each backend's name is spelled for a reader, and what language its output is. */
export const BACKENDS: Record<string, { label: string; lang: string; note: string }> = {
  lua: { label: "Lua", lang: "lua", note: "Runs on a vendored Lua 5.4, so it needs no toolchain." },
  js: { label: "JavaScript", lang: "javascript", note: "Runs through node. The only backend this site can execute." },
  native: { label: "LLVM IR", lang: "llvm", note: "Compiled to an object file and linked against a small C runtime." },
  jq: { label: "jq", lang: "jq", note: "A stream language, so keeping a dimension means iterating and collecting." },
  go: { label: "Go", lang: "go", note: "Statically typed with no runtime type information, so every type is spelled out." },
  py: { label: "Python", lang: "python", note: "Exact unbounded integers, so the 32-bit rule is emulated." },
}

export async function loadCorpus(): Promise<Corpus> {
  const res = await fetch(`${import.meta.env.BASE_URL}corpus.json`)
  if (!res.ok) throw new Error(`could not load the corpus: ${res.status} ${res.statusText}`)
  return res.json()
}
