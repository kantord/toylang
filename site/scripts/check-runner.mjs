// The site claims it runs the JavaScript backend in your browser and gets the recorded answer.
// This checks that claim against every case, using the same module the page uses, so a change
// to the shim that quietly stops matching node cannot pass unnoticed.
import { readdirSync, readFileSync } from "node:fs"

import { runJs } from "../src/lib/run-js.ts"

const corpus = JSON.parse(readFileSync(new URL("../public/corpus.json", import.meta.url)))

// The exported JSON is a committed artifact, so it can fall behind the corpus it came from.
// Comparing the names catches the common drift -- a case added or removed without re-running
// `cargo test` -- without needing a Rust toolchain to notice.
const onDisk = readdirSync(new URL("../../tests/corpus", import.meta.url))
  .filter((f) => f.endsWith(".yaml"))
  .map((f) => f.slice(0, -5))
  .sort()
const exported = corpus.cases.map((c) => c.name).sort()
if (onDisk.join() !== exported.join()) {
  const missing = onDisk.filter((n) => !exported.includes(n))
  const extra = exported.filter((n) => !onDisk.includes(n))
  console.error("public/corpus.json is stale; run `cargo test --test export_site`")
  if (missing.length) console.error(`  not exported: ${missing.join(", ")}`)
  if (extra.length) console.error(`  no longer in the corpus: ${extra.join(", ")}`)
  process.exit(1)
}

let checked = 0
let skipped = 0
const failures = []

for (const c of corpus.cases) {
  // A refusal of a program that reads stdin can live in the host's input validation
  // (src/input.rs), which runs before any backend and which the shim does not carry, so the
  // emitted code running here proves nothing either way. Refusals of input-free programs
  // (division by zero, unwrapping an absent value) are the emitted code's own and stay checked.
  // kantord/toylang#15 tracks the gap.
  if (c.expect.kind === "refusal" && c.input !== null) {
    skipped++
    continue
  }

  // Raw text either way: an `input` program's own emitted code calls JSON.parse on it, and a
  // `lines` program never was JSON to begin with.
  const result = runJs(c.emitted.js, c.input)
  checked++

  if (c.expect.kind === "refusal") {
    if (result.ok) failures.push(`${c.name}: ran and produced ${JSON.stringify(result.output)}`)
    continue
  }
  if (!result.ok) {
    failures.push(`${c.name}: refused with ${result.error}`)
  } else if (result.output !== c.expect.value) {
    failures.push(
      `${c.name}: got ${JSON.stringify(result.output)}, corpus says ${JSON.stringify(c.expect.value)}`,
    )
  }
}

if (checked === 0) {
  console.error("no cases checked, so this proves nothing")
  process.exit(1)
}
if (failures.length) {
  console.error(`${failures.length} of ${checked} cases differ in the browser shim:`)
  for (const f of failures) console.error(`  ${f}`)
  process.exit(1)
}
console.log(
  `${checked} cases: the browser shim reproduces the corpus exactly` +
    (skipped ? ` (${skipped} host-validated refusals skipped, kantord/toylang#15)` : ""),
)
