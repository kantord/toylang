export type RunResult =
  | { ok: true; output: string }
  | { ok: false; error: string }

/**
 * Run a program's emitted JavaScript in the page.
 *
 * The emitted code is written for node: it reads stdin through `require("fs")` and prints with
 * `console.log`. Both are supplied as arguments to the function rather than found on the page,
 * so the code runs unmodified -- what you read in the JavaScript tab is exactly what ran, which
 * would not be true of a version rewritten to fit the browser.
 *
 * `console.log` appends a newline, and the Rust harness captures the process's stdout, so
 * joining with newlines is what makes this comparable to the recorded output.
 */
export function runJs(code: string, input: unknown): RunResult {
  const lines: string[] = []
  const console = { log: (value: unknown) => lines.push(String(value)) }
  const require = (name: string) => {
    if (name !== "fs") throw new Error(`the emitted code asked for an unexpected module: ${name}`)
    return { readFileSync: () => JSON.stringify(input) }
  }
  try {
    new Function("require", "console", code)(require, console)
    return { ok: true, output: lines.map((line) => `${line}\n`).join("") }
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) }
  }
}
